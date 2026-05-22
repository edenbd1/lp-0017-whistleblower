// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Top-level Qt plugin object — owns the QQuickWidget that hosts the
// QML scene and exposes the WhistleblowerBackend to it as a context
// property.

#pragma once

#include <QObject>
#include <QString>
#include <QWidget>

// LogosAPI is forward-declared rather than included here so this header
// builds standalone in the IDE-only preview-app path.
class LogosAPI;
class WhistleblowerBackend;

// Basecamp's IComponent interface, declared here so the manual build
// path doesn't need the SDK header on the include path.
class IComponent {
public:
    virtual ~IComponent() = default;
    virtual QString name() const = 0;
    virtual QWidget* createWidget(LogosAPI* api) = 0;
    virtual void destroyWidget(QWidget* widget) = 0;
};

// IID required by Q_INTERFACES so moc can build the metadata table.
// Matches the IID Basecamp's PluginLoader queries via qobject_cast.
Q_DECLARE_INTERFACE(IComponent, "com.networkschool.logos.IComponent/1.0")

#define WhistleblowerPlugin_IID "com.networkschool.lp0017.WhistleblowerPlugin/1.0"

class WhistleblowerPlugin : public QObject, public IComponent {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID WhistleblowerPlugin_IID FILE "metadata.json")
    Q_INTERFACES(IComponent)

public:
    explicit WhistleblowerPlugin(QObject* parent = nullptr);
    ~WhistleblowerPlugin() override;

    QString name() const override { return QStringLiteral("whistleblower"); }
    QWidget* createWidget(LogosAPI* api) override;
    void destroyWidget(QWidget* widget) override;

private:
    WhistleblowerBackend* m_backend{nullptr};
};
