// SPDX-License-Identifier: MIT OR Apache-2.0
//
// QML-facing backend. Wires the three Logos modules we depend on
// (storage_module, delivery_module, and the lp0017_ffi cdylib for the
// on-chain registry) into a single object the QML scene can drive.

#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QUrl>

class LogosAPI;
class LogosAPIClient;
class LogosObject;

class WhistleblowerBackend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString cid READ cid NOTIFY cidChanged)
    Q_PROPERTY(QString lastTxHash READ lastTxHash NOTIFY lastTxHashChanged)
    Q_PROPERTY(QString selectedFile READ selectedFile WRITE setSelectedFile NOTIFY selectedFileChanged)

public:
    explicit WhistleblowerBackend(LogosAPI* api, QObject* parent = nullptr);
    ~WhistleblowerBackend() override;

    QString status() const { return m_status; }
    bool busy() const { return m_busy; }
    QString cid() const { return m_cid; }
    QString lastTxHash() const { return m_lastTxHash; }
    QString selectedFile() const { return m_selectedFile; }
    void setSelectedFile(const QString& path);

public slots:
    // Upload the currently selected file to storage_module, then
    // build the envelope and publish via delivery_module. The
    // anchoring step is a separate explicit action — the spec keeps
    // upload and on-chain anchor distinct.
    void publish(const QString& title, const QString& description, const QStringList& tags);

    // Anchor the last published CID on-chain via the lp0017_ffi cdylib.
    // No-op if there is no last CID.
    void anchorLast();

signals:
    void statusChanged();
    void busyChanged();
    void cidChanged();
    void lastTxHashChanged();
    void selectedFileChanged();

private:
    void setStatus(const QString& s);
    void setBusy(bool b);

    LogosAPI* m_api{nullptr};
    LogosAPIClient* m_storageClient{nullptr};
    LogosAPIClient* m_deliveryClient{nullptr};
    LogosObject* m_storageObject{nullptr};
    LogosObject* m_deliveryObject{nullptr};

    QString m_status{QStringLiteral("idle")};
    bool m_busy{false};
    QString m_cid;
    QString m_lastTxHash;
    QString m_selectedFile;
    QString m_lastMetadataHash;
    qint64 m_lastTimestamp{0};
};
