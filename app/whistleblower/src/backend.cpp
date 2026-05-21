// SPDX-License-Identifier: MIT OR Apache-2.0
#include "backend.h"

#include <QCryptographicHash>
#include <QDateTime>
#include <QDebug>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QVariant>
#include <QVariantList>

// LogosAPI / LogosAPIClient / LogosObject are forward-declared in the
// header. In a real Basecamp build these are provided by logos-cpp-sdk;
// the manual build path stubs them out so QML iteration works without
// the full SDK on PATH.
//
// Keeping the includes here (rather than in backend.h) means the
// plugin.h public surface stays SDK-agnostic.

#ifdef LOGOS_SDK_AVAILABLE
#  include <logos_api.h>
#  include <logos_api_client.h>
#  include <logos_object.h>
#else
// Stub types so the manual build compiles. The real SDK provides far
// more, but this is enough to construct/exercise the backend in the
// preview app.
class LogosAPI {
public:
    virtual ~LogosAPI() = default;
    virtual class LogosAPIClient* getClient(const QString&) { return nullptr; }
};
class LogosAPIClient {
public:
    QVariant invokeRemoteMethod(const QString&, const QString&, const QVariantList&, int = 30000) {
        return {};
    }
    void invokeRemoteMethodAsync(const QString&, const QString&, const QVariantList&,
                                 std::function<void(QVariant)> /*cb*/) {}
    class LogosObject* requestObject(const QString&) { return nullptr; }
    void onEvent(class LogosObject*, const QString&, std::function<void(QString,QVariantList)>) {}
};
class LogosObject {};
#endif

namespace {
constexpr const char* kStorageModule = "storage_module";
constexpr const char* kDeliveryModule = "delivery_module";
constexpr const char* kTopic = "/whistleblower/1/document-broadcast/json";

QByteArray canonicalMetadataJson(const QString& title,
                                 const QString& description,
                                 const QString& contentType,
                                 quint64 sizeBytes,
                                 const QStringList& tags) {
    // Same shape as crates/indexing/src/envelope.rs::canonical_metadata_hash.
    // Hand-rolled JSON because QJsonDocument doesn't promise key order.
    QStringList parts;
    auto esc = [](const QString& s) {
        QString out;
        out.reserve(s.size() + 2);
        out.append('"');
        for (QChar c : s) {
            ushort u = c.unicode();
            switch (u) {
                case '"': out.append("\\\""); break;
                case '\\': out.append("\\\\"); break;
                case '\n': out.append("\\n"); break;
                case '\r': out.append("\\r"); break;
                case '\t': out.append("\\t"); break;
                default:
                    if (u < 0x20)
                        out.append(QStringLiteral("\\u%1").arg(u, 4, 16, QLatin1Char('0')));
                    else
                        out.append(c);
            }
        }
        out.append('"');
        return out;
    };
    if (!title.isEmpty()) parts << QStringLiteral("\"title\":%1").arg(esc(title));
    if (!description.isEmpty()) parts << QStringLiteral("\"description\":%1").arg(esc(description));
    if (!contentType.isEmpty()) parts << QStringLiteral("\"content_type\":%1").arg(esc(contentType));
    if (sizeBytes > 0) parts << QStringLiteral("\"size_bytes\":%1").arg(sizeBytes);
    if (!tags.isEmpty()) {
        QStringList esc_tags;
        for (const auto& t : tags) esc_tags << esc(t);
        parts << QStringLiteral("\"tags\":[%1]").arg(esc_tags.join(','));
    }
    return QStringLiteral("{%1}").arg(parts.join(',')).toUtf8();
}

QString sha256Hex(const QByteArray& bytes) {
    return QString::fromLatin1(QCryptographicHash::hash(bytes, QCryptographicHash::Sha256).toHex());
}
}  // namespace

WhistleblowerBackend::WhistleblowerBackend(LogosAPI* api, QObject* parent)
    : QObject(parent), m_api(api) {
    if (m_api) {
        m_storageClient = m_api->getClient(kStorageModule);
        m_deliveryClient = m_api->getClient(kDeliveryModule);
        if (m_storageClient) {
            m_storageObject = m_storageClient->requestObject(kStorageModule);
            // start() is idempotent — calling on a running module is a no-op.
            m_storageClient->invokeRemoteMethod(
                kStorageModule, QStringLiteral("start"), {}, 30000);
        }
        if (m_deliveryClient) {
            m_deliveryObject = m_deliveryClient->requestObject(kDeliveryModule);
            m_deliveryClient->invokeRemoteMethod(
                kDeliveryModule, QStringLiteral("init"),
                {QStringLiteral(R"({"logLevel":"INFO","mode":"Core","preset":"logos.dev"})")},
                30000);
            m_deliveryClient->invokeRemoteMethod(
                kDeliveryModule, QStringLiteral("start"), {}, 30000);
        }
    } else {
        qInfo() << "WhistleblowerBackend: no LogosAPI — running in preview-only mode.";
    }
}

WhistleblowerBackend::~WhistleblowerBackend() = default;

void WhistleblowerBackend::setSelectedFile(const QString& path) {
    QString cleaned = path;
    if (cleaned.startsWith(QStringLiteral("file://"))) {
        cleaned.remove(0, 7);
    }
    if (m_selectedFile != cleaned) {
        m_selectedFile = cleaned;
        emit selectedFileChanged();
        setStatus(QStringLiteral("selected: ") + QFileInfo(cleaned).fileName());
    }
}

void WhistleblowerBackend::publish(const QString& title,
                                   const QString& description,
                                   const QStringList& tags) {
    if (m_selectedFile.isEmpty()) {
        setStatus(QStringLiteral("no file selected"));
        return;
    }
    if (!m_storageClient || !m_deliveryClient) {
        setStatus(QStringLiteral("LogosAPI not wired — preview only"));
        return;
    }

    setBusy(true);
    setStatus(QStringLiteral("uploading to Logos Storage…"));

    QFileInfo info(m_selectedFile);
    const QString displayTitle = title.isEmpty() ? info.fileName() : title;
    const quint64 sizeBytes = static_cast<quint64>(info.size());
    const QString contentType = QStringLiteral("application/octet-stream");

    const QByteArray canonical =
        canonicalMetadataJson(displayTitle, description, contentType, sizeBytes, tags);
    m_lastMetadataHash = QStringLiteral("v1:") + sha256Hex(canonical);
    m_lastTimestamp = QDateTime::currentSecsSinceEpoch();

    // storage_module.uploadUrl is async; the CID comes back on the
    // storageUploadDone event. The completion lambda below builds and
    // broadcasts the envelope once we have the CID.
    auto onUploadDone = [this, displayTitle, description, contentType, sizeBytes, tags]
                        (QString /*origin*/, QVariantList data) {
        QString resultingCid;
        for (const QVariant& v : data) {
            if (v.canConvert<QString>()) {
                const QString s = v.toString();
                if (s.startsWith('z') || s.startsWith(QStringLiteral("bafy"))) {
                    resultingCid = s;
                    break;
                }
            }
        }
        if (resultingCid.isEmpty()) {
            setStatus(QStringLiteral("upload finished, but no CID found in event"));
            setBusy(false);
            return;
        }
        m_cid = resultingCid;
        emit cidChanged();
        setStatus(QStringLiteral("uploaded; broadcasting…"));

        QJsonObject env;
        env[QStringLiteral("v")] = 1;
        env[QStringLiteral("cid")] = m_cid;
        env[QStringLiteral("metadata_hash")] = m_lastMetadataHash;
        env[QStringLiteral("timestamp")] = static_cast<qint64>(m_lastTimestamp);
        env[QStringLiteral("title")] = displayTitle;
        if (!description.isEmpty()) env[QStringLiteral("description")] = description;
        if (!contentType.isEmpty()) env[QStringLiteral("content_type")] = contentType;
        if (sizeBytes > 0) env[QStringLiteral("size_bytes")] = static_cast<qint64>(sizeBytes);
        if (!tags.isEmpty()) {
            QJsonArray arr;
            for (const auto& t : tags) arr.append(t);
            env[QStringLiteral("tags")] = arr;
        }
        const QByteArray payloadBytes = QJsonDocument(env).toJson(QJsonDocument::Compact);
        const QString payloadB64 = QString::fromLatin1(payloadBytes.toBase64());

        m_deliveryClient->invokeRemoteMethodAsync(
            kDeliveryModule, QStringLiteral("send"),
            QVariantList{QString::fromLatin1(kTopic), payloadB64},
            [this](QVariant) {
                setStatus(QStringLiteral("broadcast sent"));
                setBusy(false);
            });
    };

    m_storageClient->onEvent(m_storageObject, QStringLiteral("storageUploadDone"),
                             onUploadDone);
    m_storageClient->invokeRemoteMethodAsync(
        kStorageModule, QStringLiteral("uploadUrl"),
        QVariantList{QVariant::fromValue(QUrl::fromLocalFile(m_selectedFile)),
                     QVariant::fromValue(static_cast<int>(64 * 1024))},
        [](QVariant) {});
}

void WhistleblowerBackend::anchorLast() {
    if (m_cid.isEmpty()) {
        setStatus(QStringLiteral("no CID to anchor — publish first"));
        return;
    }
    // The full anchor path goes through the lp0017_ffi cdylib. The
    // wire format mirrors the FFI's IndexBatchRequest (see
    // crates/ffi/src/lib.rs).
    setStatus(QStringLiteral("anchoring on-chain (FFI)…"));
    setBusy(true);

    // Concrete FFI invocation lives behind a separate build module
    // (linked when LOGOS_MODULE_BUILDER_ROOT is set). The preview
    // build short-circuits with a placeholder so QML can be iterated
    // without the cdylib next to the binary.
    setStatus(QStringLiteral("anchor request prepared (FFI link wired by framework build)"));
    m_lastTxHash = QStringLiteral("pending — link lp0017_ffi to surface tx_hash");
    emit lastTxHashChanged();
    setBusy(false);
}

void WhistleblowerBackend::setStatus(const QString& s) {
    if (m_status != s) {
        m_status = s;
        emit statusChanged();
    }
}

void WhistleblowerBackend::setBusy(bool b) {
    if (m_busy != b) {
        m_busy = b;
        emit busyChanged();
    }
}
