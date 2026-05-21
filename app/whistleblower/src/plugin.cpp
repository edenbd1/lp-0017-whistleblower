// SPDX-License-Identifier: MIT OR Apache-2.0
#include "plugin.h"
#include "backend.h"

#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickWidget>
#include <QUrl>

WhistleblowerPlugin::WhistleblowerPlugin(QObject* parent) : QObject(parent) {}

WhistleblowerPlugin::~WhistleblowerPlugin() = default;

QWidget* WhistleblowerPlugin::createWidget(LogosAPI* api) {
    m_backend = new WhistleblowerBackend(api, this);

    auto* view = new QQuickWidget();
    view->engine()->rootContext()->setContextProperty(
        QStringLiteral("backend"), m_backend);
    view->setResizeMode(QQuickWidget::SizeRootObjectToView);
    view->setSource(QUrl(QStringLiteral("qrc:/qml/Main.qml")));
    return view;
}

void WhistleblowerPlugin::destroyWidget(QWidget* widget) {
    if (widget) {
        widget->deleteLater();
    }
}
