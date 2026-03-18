import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

ApplicationWindow {
    id: window
    width: 1440
    height: 920
    visible: true
    title: "Zen Session Restore"
    color: palette.window

    property var backupsModel: []
    property var activeBackup: ({})
    property var backendRef: typeof backend !== "undefined" ? backend : null

    function parseBackups() {
        if (!backendRef) {
            backupsModel = []
            return
        }
        try {
            backupsModel = JSON.parse(backendRef.backups_json)
        } catch (error) {
            backupsModel = []
        }
    }

    function parseActiveBackup() {
        if (!backendRef) {
            activeBackup = ({})
            return
        }
        try {
            activeBackup = JSON.parse(backendRef.active_backup_json)
        } catch (error) {
            activeBackup = ({})
        }
    }

    function tabBadgeText(tab) {
        let parts = []
        if (tab.pinned)
            parts.push("Pinned")
        if (tab.essential)
            parts.push("Essential")
        if (!tab.restorable)
            parts.push("Unsupported")
        return parts.join(" • ")
    }

    function localizedSnapshotLabel(savedAtMs, fallbackLabel) {
        if (savedAtMs === undefined || savedAtMs === null)
            return fallbackLabel

        const snapshotDate = new Date(savedAtMs)
        if (isNaN(snapshotDate.getTime()))
            return fallbackLabel

        return Qt.locale().toString(snapshotDate, Locale.ShortFormat)
    }

    menuBar: MenuBar {
        Menu {
            title: "&Help"

            Action {
                text: "About Restore Zen Session..."
                onTriggered: aboutDialog.open()
            }
        }
    }

    Connections {
        target: backendRef
        function onBackups_jsonChanged() { parseBackups() }
        function onActive_backup_jsonChanged() { parseActiveBackup() }
    }

    Component.onCompleted: {
        parseBackups()
        parseActiveBackup()
        if (backendRef && backendRef.show_about_on_startup)
            aboutDialog.open()
        if (backendRef && backendRef.should_prompt_for_profile)
            profileFolderDialog.open()
    }

    header: Rectangle {
        implicitHeight: 70
        color: palette.window
        border.color: palette.mid

        GridLayout {
            anchors.fill: parent
            anchors.margins: 12
            columns: 2
            rowSpacing: 0
            columnSpacing: 12

            Label {
                text: "Zen Session Restore"
                font.weight: Font.DemiBold
                color: palette.windowText
                Layout.column: 0
                Layout.row: 0
                Layout.fillWidth: true
            }

            RowLayout {
                Layout.column: 1
                Layout.row: 0
                Layout.rowSpan: 2
                Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                spacing: 10

                Frame {
                    padding: 8
                    Layout.alignment: Qt.AlignVCenter

                    contentItem: Label {
                        text: backendRef && backendRef.zen_running ? "Profile in use" : "Profile available"
                        color: palette.windowText
                        font.weight: Font.Medium
                    }
                }

                Button {
                    text: "Refresh"
                    enabled: !!backendRef
                    onClicked: if (backendRef) backendRef.refresh()
                    Layout.alignment: Qt.AlignVCenter
                }

                Button {
                    text: "Open Profile Folder"
                    enabled: !!backendRef
                    onClicked: profileFolderDialog.open()
                    Layout.alignment: Qt.AlignVCenter
                }
            }

            Label {
                text: backendRef && backendRef.profile_path.length > 0 ? backendRef.profile_path : "No Zen profile loaded"
                elide: Text.ElideMiddle
                color: palette.placeholderText
                Layout.column: 0
                Layout.row: 1
                Layout.columnSpan: 1
                Layout.fillWidth: true
                topPadding: -2
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        anchors.topMargin: 12
        spacing: 12

        Frame {
            Layout.fillWidth: true
            padding: 12

            contentItem: RowLayout {
                spacing: 12

                Label {
                    Layout.fillWidth: true
                    text: backendRef ? backendRef.status_text : "Loading Zen backups..."
                    wrapMode: Text.WordWrap
                    color: palette.windowText
                }

                CheckBox {
                    id: launchAfterRestore
                    text: "Launch Zen after restore"
                    checked: backendRef ? backendRef.launch_after_restore : false
                    enabled: !!backendRef
                    onToggled: if (backendRef) backendRef.set_launch_after_restore(checked)
                }

                Button {
                    text: "Restore Selected"
                    highlighted: true
                    enabled: !!backendRef && !!activeBackup.selectedTabs && activeBackup.selectedTabs > 0
                    onClicked: selectiveRestoreDialog.open()
                }

                Button {
                    text: "Restore Full Backup"
                    enabled: !!backendRef && !!activeBackup.fileName
                    onClicked: fullRestoreDialog.open()
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal

            Frame {
                SplitView.minimumWidth: 260
                SplitView.preferredWidth: 300
                SplitView.maximumWidth: 340
                padding: 10

                contentItem: ColumnLayout {
                    spacing: 10

                    Label {
                        text: "Snapshots"
                        font.weight: Font.DemiBold
                        color: palette.windowText
                    }

                    Label {
                        text: "Newest first"
                        color: palette.placeholderText
                    }

                    ListView {
                        id: snapshotsList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 8
                        model: backupsModel
                        rightMargin: snapshotsScrollBar.visible ? snapshotsScrollBar.width + 8 : 0
                        ScrollBar.vertical: ScrollBar {
                            id: snapshotsScrollBar
                            policy: ScrollBar.AsNeeded
                            visible: snapshotsList.contentHeight > snapshotsList.height
                        }

                        delegate: ItemDelegate {
                            required property var modelData
                            width: snapshotsList.width - snapshotsList.rightMargin
                            height: 88
                            padding: 0

                            background: Rectangle {
                                radius: 10
                                color: modelData.active ? palette.highlight : palette.base
                                border.color: modelData.active ? palette.highlight : palette.mid
                                border.width: modelData.active ? 2 : 1
                            }

                            contentItem: ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 12
                                spacing: 4

                                Label {
                                    Layout.fillWidth: true
                                    text: window.localizedSnapshotLabel(modelData.savedAtMs, modelData.snapshotLabel)
                                    elide: Text.ElideRight
                                    font.weight: Font.DemiBold
                                    color: modelData.active ? palette.highlightedText : palette.windowText
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.fileName
                                    elide: Text.ElideMiddle
                                    color: modelData.active ? palette.highlightedText : palette.placeholderText
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.collections + " spaces • " + modelData.tabs + " tabs"
                                    elide: Text.ElideRight
                                    color: modelData.active ? palette.highlightedText : palette.placeholderText
                                }
                            }

                            onClicked: if (backendRef) backendRef.select_backup(modelData.index)
                        }
                    }
                }
            }

            Frame {
                SplitView.fillWidth: true
                padding: 12

                contentItem: ColumnLayout {
                    spacing: 12

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: activeBackup.fileName ? window.localizedSnapshotLabel(activeBackup.savedAtMs, activeBackup.snapshotLabel) : "Select a snapshot"
                            elide: Text.ElideMiddle
                            font.weight: Font.DemiBold
                            color: palette.windowText
                        }

                        Label {
                            text: activeBackup.snapshotLabel ? activeBackup.snapshotLabel : "Choose a snapshot from the sidebar to inspect its spaces and tabs."
                            color: palette.placeholderText
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        visible: !!activeBackup.fileName

                        Repeater {
                            model: [
                                { "label": "Tabs", "value": activeBackup.totalTabs || 0 },
                                { "label": "Selected", "value": activeBackup.selectedTabs || 0 },
                                { "label": "Spaces", "value": activeBackup.collections ? activeBackup.collections.length : 0 }
                            ]

                            delegate: Frame {
                                required property var modelData
                                padding: 10
                                Layout.preferredWidth: 116

                                contentItem: ColumnLayout {
                                    spacing: 2

                                    Label {
                                        Layout.fillWidth: true
                                        text: modelData.label
                                        horizontalAlignment: Text.AlignHCenter
                                        color: palette.placeholderText
                                    }

                                    Label {
                                        Layout.fillWidth: true
                                        text: modelData.value
                                        horizontalAlignment: Text.AlignHCenter
                                        color: palette.windowText
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }
                        }

                        Item { Layout.fillWidth: true }
                    }

                    ScrollView {
                        id: detailsScroll
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        padding: 4
                        clip: true

                        Item {
                            width: detailsScroll.availableWidth
                            implicitHeight: detailsColumn.implicitHeight

                            ColumnLayout {
                                id: detailsColumn
                                width: parent.width
                                spacing: 10

                                Repeater {
                                    model: activeBackup.collections ? activeBackup.collections : []

                                    delegate: Frame {
                                        required property var modelData
                                        property int collectionIndex: modelData.index
                                        Layout.fillWidth: true
                                        padding: 12

                                        contentItem: ColumnLayout {
                                            width: parent.width
                                            spacing: 10

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 1

                                                    Label {
                                                        text: modelData.title
                                                        font.weight: Font.DemiBold
                                                        color: palette.windowText
                                                    }

                                                    Label {
                                                        text: modelData.selectedCount + " selected • " + modelData.tabCount + " tabs"
                                                        color: palette.placeholderText
                                                    }
                                                }

                                                Button {
                                                    text: modelData.selectedCount === modelData.tabCount ? "Deselect All" : "Select All"
                                                    flat: true
                                                    Layout.alignment: Qt.AlignTop
                                                    onClicked: if (backendRef) backendRef.toggle_collection(modelData.index, modelData.selectedCount !== modelData.tabCount)
                                                }
                                            }

                                            Repeater {
                                                model: modelData.tabs

                                                delegate: Frame {
                                                    required property var modelData
                                                    Layout.fillWidth: true
                                                    padding: 10
                                                    clip: true
                                                    implicitHeight: contentLayout.implicitHeight + 20

                                                    background: Rectangle {
                                                        radius: 8
                                                        color: "transparent"
                                                        border.color: modelData.selected ? palette.highlight : palette.mid
                                                        border.width: modelData.selected ? 2 : 1
                                                    }

                                                    contentItem: RowLayout {
                                                        id: contentLayout
                                                        width: parent.width
                                                        spacing: 10

                                                        CheckBox {
                                                            Layout.alignment: Qt.AlignTop
                                                            checked: modelData.selected
                                                            enabled: modelData.restorable
                                                            onToggled: if (backendRef) backendRef.toggle_tab(collectionIndex, modelData.index, checked)
                                                        }

                                                        ColumnLayout {
                                                            Layout.fillWidth: true
                                                            Layout.alignment: Qt.AlignTop
                                                            spacing: 3

                                                            RowLayout {
                                                                Layout.fillWidth: true
                                                                spacing: 10

                                                                Label {
                                                                    Layout.fillWidth: true
                                                                    text: modelData.title
                                                                    elide: Text.ElideRight
                                                                    color: palette.windowText
                                                                    font.weight: Font.Medium
                                                                    maximumLineCount: 1
                                                                }

                                                                Frame {
                                                                    visible: window.tabBadgeText(modelData).length > 0
                                                                    Layout.alignment: Qt.AlignTop
                                                                    padding: 6
                                                                    implicitWidth: badgeLabel.implicitWidth + leftPadding + rightPadding

                                                                    Label {
                                                                        id: badgeLabel
                                                                        anchors.centerIn: parent
                                                                        width: parent.width - parent.leftPadding - parent.rightPadding
                                                                        text: window.tabBadgeText(modelData)
                                                                        horizontalAlignment: Text.AlignHCenter
                                                                        elide: Text.ElideRight
                                                                        color: palette.windowText
                                                                        font.weight: Font.Medium
                                                                    }
                                                                }
                                                            }

                                                            Label {
                                                                Layout.fillWidth: true
                                                                text: modelData.url ? modelData.url : "Unsupported or missing URL"
                                                                elide: Text.ElideRight
                                                                maximumLineCount: 1
                                                                clip: true
                                                                color: palette.placeholderText
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            Item {
                                                width: 1
                                                height: 6
                                            }
                                        }
                                    }
                                }

                                Item {
                                    width: 1
                                    height: 12
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Dialog {
        id: aboutDialog
        modal: true
        title: "About Restore Zen Session"
        standardButtons: Dialog.Ok

        contentItem: ColumnLayout {
            width: 460
            spacing: 10

            Label {
                text: "Restore Zen Session"
                font.weight: Font.DemiBold
                color: palette.windowText
            }

            Label {
                text: "Version 0.3"
                color: palette.windowText
            }

            Label {
                text: "Author: Pete Vagiakos"
                color: palette.windowText
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                textFormat: Text.RichText
                onLinkActivated: link => Qt.openUrlExternally(link)
                text: "GitHub: <a href=\"https://github.com/Kombatant/restore_zen_session\">https://github.com/Kombatant/restore_zen_session</a><br>Issues: <a href=\"https://github.com/Kombatant/restore_zen_session/issues\">https://github.com/Kombatant/restore_zen_session/issues</a>"
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: palette.placeholderText
                text: "Report bugs or restoration issues through the GitHub issues page."
            }
        }
    }

    Dialog {
        id: fullRestoreDialog
        modal: true
        title: "Restore Full Backup"
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: if (backendRef) backendRef.restore_full_backup()

        contentItem: Label {
            width: 400
            wrapMode: Text.WordWrap
            text: "This will overwrite zen-sessions.jsonlz4 with the selected snapshot. Zen should be closed first. Continue?"
        }
    }

    Dialog {
        id: selectiveRestoreDialog
        modal: true
        title: "Restore Selected Tabs"
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: if (backendRef) backendRef.restore_selected()

        contentItem: Label {
            width: 430
            wrapMode: Text.WordWrap
            text: "This will write a filtered zen-sessions.jsonlz4 containing only the currently selected tabs and spaces. Zen should be closed first. Continue?"
        }
    }

    FolderDialog {
        id: profileFolderDialog
        title: "Choose Your Zen Profile Folder"

        onAccepted: {
            if (backendRef)
                backendRef.set_profile_path(selectedFolder.toLocalFile())
        }
    }
}
