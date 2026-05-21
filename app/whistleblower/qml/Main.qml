// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Whistleblower main pane.
//
// Three actions, plus a status panel. File picker → publish (upload +
// broadcast) → anchor (on-chain). `backend` is the WhistleblowerBackend
// instance injected as a context property by plugin.cpp.

import QtQuick 6
import QtQuick.Controls 6
import QtQuick.Layouts 6
import QtQuick.Dialogs 6

Rectangle {
    id: root
    color: "#0d1117"
    anchors.fill: parent

    FileDialog {
        id: filePicker
        title: "Select a document to publish"
        onAccepted: backend.selectedFile = filePicker.selectedFile.toString()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 16

        Label {
            text: "Whistleblower"
            color: "#f0f6fc"
            font.pixelSize: 24
            font.bold: true
        }

        Label {
            text: "Censorship-resistant document upload + on-chain anchoring (LP-0017)"
            color: "#8b949e"
            font.pixelSize: 13
        }

        // ── File picker row ─────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Button {
                text: "Choose file…"
                onClicked: filePicker.open()
                enabled: !backend.busy
            }

            Label {
                text: backend.selectedFile === ""
                      ? "(no file selected)"
                      : backend.selectedFile
                color: "#c9d1d9"
                font.family: "Menlo, monospace"
                font.pixelSize: 12
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
        }

        // ── Metadata form ───────────────────────────────────────────
        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 12
            rowSpacing: 8

            Label { text: "Title"; color: "#c9d1d9" }
            TextField {
                id: titleField
                Layout.fillWidth: true
                placeholderText: "Defaults to filename"
                enabled: !backend.busy
            }

            Label { text: "Description"; color: "#c9d1d9" }
            TextField {
                id: descField
                Layout.fillWidth: true
                placeholderText: "(optional)"
                enabled: !backend.busy
            }

            Label { text: "Tags"; color: "#c9d1d9" }
            TextField {
                id: tagsField
                Layout.fillWidth: true
                placeholderText: "comma,separated"
                enabled: !backend.busy
            }
        }

        // ── Action buttons ──────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Button {
                text: "Publish (upload + broadcast)"
                enabled: !backend.busy && backend.selectedFile !== ""
                onClicked: {
                    const tags = tagsField.text.trim() === ""
                                 ? []
                                 : tagsField.text.split(",").map(t => t.trim()).filter(t => t.length > 0);
                    backend.publish(titleField.text, descField.text, tags)
                }
            }

            Button {
                text: "Anchor on-chain"
                enabled: !backend.busy && backend.cid !== ""
                onClicked: backend.anchorLast()
            }

            Item { Layout.fillWidth: true }

            BusyIndicator {
                running: backend.busy
                Layout.preferredWidth: 28
                Layout.preferredHeight: 28
            }
        }

        // ── Status / results panel ──────────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 140
            color: "#161b22"
            border.color: "#30363d"
            radius: 6

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 6

                Label {
                    text: "Status: " + backend.status
                    color: "#8b949e"
                    font.pixelSize: 12
                }
                Label {
                    text: "CID: " + (backend.cid === "" ? "—" : backend.cid)
                    color: "#7ee787"
                    font.family: "Menlo, monospace"
                    font.pixelSize: 12
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }
                Label {
                    text: "tx_hash: " + (backend.lastTxHash === "" ? "—" : backend.lastTxHash)
                    color: "#79c0ff"
                    font.family: "Menlo, monospace"
                    font.pixelSize: 12
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }
            }
        }

        Item { Layout.fillHeight: true }
    }
}
