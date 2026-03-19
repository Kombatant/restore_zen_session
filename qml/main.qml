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
    property bool syncInFlight: false
    property string repositoryReadmeUrl: "https://github.com/Kombatant/restore_zen_session#readme"

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
        implicitHeight: 84
        color: palette.window
        border.color: palette.mid

        RowLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 16

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Label {
                    text: "Zen Session Restore"
                    font.pixelSize: 24
                    font.weight: Font.DemiBold
                    color: palette.windowText
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Rectangle {
                        radius: 7
                        color: backendRef && backendRef.zen_running ? palette.highlight : palette.base
                        border.color: backendRef && backendRef.zen_running ? palette.highlight : palette.mid
                        implicitHeight: profileStatus.implicitHeight + 10
                        implicitWidth: profileStatus.implicitWidth + 18

                        Label {
                            id: profileStatus
                            anchors.centerIn: parent
                            text: backendRef && backendRef.zen_running ? "Profile in use" : "Profile available"
                            color: backendRef && backendRef.zen_running ? palette.highlightedText : palette.windowText
                            font.weight: Font.Medium
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        text: backendRef && backendRef.profile_path.length > 0 ? backendRef.profile_path : "No Zen profile loaded"
                        elide: Text.ElideMiddle
                        color: palette.placeholderText
                    }
                }
            }

            RowLayout {
                spacing: 10

                Button {
                    text: "Refresh"
                    enabled: !!backendRef
                    onClicked: if (backendRef) backendRef.refresh()
                }

                Button {
                    text: "Open Profile Folder"
                    enabled: !!backendRef
                    onClicked: profileFolderDialog.open()
                }

                Button {
                    text: syncDrawer.visible ? "Hide Cloud Sync ◂" : "Open Cloud Sync ▸"
                    enabled: !!backendRef
                    onClicked: {
                        if (syncDrawer.visible)
                            syncDrawer.close()
                        else
                            syncDrawer.open()
                    }
                }
            }
        }
    }

    Drawer {
        id: syncDrawer
        edge: Qt.RightEdge
        width: Math.min(window.width * 0.33, 420)
        height: window.height
        modal: false

        background: Rectangle {
            color: palette.window
            border.color: palette.mid
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        text: "Cloud Sync"
                        font.pixelSize: 22
                        font.weight: Font.DemiBold
                        color: palette.windowText
                    }

                    Label {
                        text: "Google Drive / Google One"
                        color: palette.placeholderText
                    }
                }

                ToolButton {
                    text: "\u00d7"
                    font.pixelSize: 22
                    font.weight: Font.Light
                    implicitWidth: 36
                    implicitHeight: 36
                    Layout.alignment: Qt.AlignTop | Qt.AlignRight
                    onClicked: syncDrawer.close()
                }
            }

            Label {
                Layout.fillWidth: true
                text: backendRef ? backendRef.cloud_sync_status_text : "Configure Google Drive sync for zen-sessions-backup."
                wrapMode: Text.WordWrap
                color: palette.windowText
            }

            Rectangle {
                Layout.fillWidth: true
                visible: !backendRef || !backendRef.google_oauth_ready
                radius: 12
                color: Qt.rgba(palette.highlight.r, palette.highlight.g, palette.highlight.b, 0.12)
                border.color: palette.highlight
                implicitHeight: googleConfigWarning.implicitHeight + 24

                Label {
                    id: googleConfigWarning
                    anchors.fill: parent
                    anchors.margins: 12
                    wrapMode: Text.WordWrap
                    color: palette.windowText
                    textFormat: Text.RichText
                    linkColor: palette.link
                    text: "Cloud Sync is disabled because <b>google.json</b> was not found next to the app. " +
                        "Add your Google Client ID and Client Secret in that file, then reopen the app. " +
                        "See the GitHub README for setup details and the required file format: " +
                        "<a href=\"" + repositoryReadmeUrl + "\">" + repositoryReadmeUrl + "</a>"
                    onLinkActivated: function(link) { Qt.openUrlExternally(link) }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                radius: 12
                color: palette.base
                border.color: palette.mid
                implicitHeight: syncColumn.implicitHeight + 24

                ColumnLayout {
                    id: syncColumn
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 12

                    CheckBox {
                        text: "Sync backup folder"
                        checked: backendRef ? backendRef.cloud_sync_enabled : false
                        enabled: !!backendRef && backendRef.google_oauth_ready
                        onToggled: if (backendRef) backendRef.set_cloud_sync_enabled(checked)
                    }

                    Item {
                        Layout.fillWidth: true
                        implicitHeight: actionFlow.implicitHeight

                        Flow {
                            id: actionFlow
                            width: parent.width
                            spacing: 10

                            Button {
                                text: "Sync Now"
                                highlighted: true
                                enabled: !!backendRef && backendRef.google_oauth_ready && backendRef.cloud_sync_enabled && backendRef.google_auth_connected && !syncInFlight
                                onClicked: {
                                    if (!backendRef)
                                        return
                                    syncInFlight = true
                                    Qt.callLater(function() {
                                        backendRef.sync_cloud_backup()
                                        syncInFlight = false
                                    })
                                }
                            }

                            Button {
                                text: backendRef && backendRef.google_auth_connected ? "Reconnect Google Drive" : "Connect Google Drive"
                                enabled: !!backendRef && backendRef.google_oauth_ready
                                onClicked: if (backendRef) backendRef.connect_google_drive()
                            }

                            Button {
                                text: "Disconnect"
                                enabled: !!backendRef && backendRef.google_oauth_ready && backendRef.google_auth_connected
                                onClicked: if (backendRef) backendRef.disconnect_google_drive()
                            }
                        }
                    }

                    RowLayout {
                        spacing: 10

                        Label {
                            text: "Retention"
                            color: palette.windowText
                            Layout.alignment: Qt.AlignVCenter
                        }

                        SpinBox {
                            from: 1
                            to: 12
                            value: backendRef ? backendRef.retention_months : 3
                            enabled: !!backendRef && backendRef.google_oauth_ready
                            onValueModified: if (backendRef) backendRef.set_retention_months(value)
                        }

                        Label {
                            text: "months"
                            color: palette.placeholderText
                            Layout.alignment: Qt.AlignVCenter
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: palette.windowText
                        text: backendRef && backendRef.google_oauth_ready
                            ? (backendRef.google_auth_connected
                                ? "Google Drive is connected for this app."
                                : "Connect in your browser to grant Google Drive access.")
                            : "Google Drive sign-in is disabled until google.json is added beside the app."
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: palette.placeholderText
                        text: "The app opens your browser for Google sign-in, stores the refresh token locally, and mirrors zen-sessions-backup into Google Drive under Backup/Zen."
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        visible: syncInFlight

                        ProgressBar {
                            Layout.fillWidth: true
                            indeterminate: true
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: palette.windowText
                            text: "Syncing backups with Google Drive..."
                        }
                    }
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 16

        Rectangle {
            Layout.fillWidth: true
            radius: 12
            color: palette.base
            border.color: palette.mid
            implicitHeight: statusRow.implicitHeight + 20

            RowLayout {
                id: statusRow
                anchors.fill: parent
                anchors.margins: 12
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: "Status"
                        font.weight: Font.DemiBold
                        color: palette.windowText
                    }

                    Label {
                        Layout.fillWidth: true
                        text: backendRef ? backendRef.status_text : "Loading Zen backups..."
                        wrapMode: Text.WordWrap
                        color: palette.placeholderText
                    }
                }

                Rectangle {
                    radius: 8
                    color: "transparent"
                    border.color: palette.mid
                    implicitHeight: syncStatusLabel.implicitHeight + 12
                    implicitWidth: syncStatusLabel.implicitWidth + 20
                    visible: !!backendRef

                    Label {
                        id: syncStatusLabel
                        anchors.centerIn: parent
                        text: syncInFlight
                            ? "Syncing"
                            : (backendRef && backendRef.cloud_sync_enabled ? "Sync enabled" : "Sync off")
                        color: palette.windowText
                        font.weight: Font.Medium
                    }
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal

            Frame {
                SplitView.minimumWidth: 280
                SplitView.preferredWidth: 320
                SplitView.maximumWidth: 380
                padding: 0

                background: Rectangle {
                    radius: 14
                    color: palette.base
                    border.color: palette.mid
                }

                contentItem: ColumnLayout {
                    spacing: 0

                    Item {
                        Layout.fillWidth: true
                        implicitHeight: snapshotsHeader.implicitHeight + 30

                        ColumnLayout {
                            id: snapshotsHeader
                            anchors.fill: parent
                            anchors.margins: 16
                            spacing: 4

                            Label {
                                text: "Snapshots"
                                font.pixelSize: 20
                                font.weight: Font.DemiBold
                                color: palette.windowText
                            }

                            Label {
                                text: backupsModel.length > 0 ? "Choose a backup to inspect before restoring." : "No snapshots found in the current profile."
                                wrapMode: Text.WordWrap
                                color: palette.placeholderText
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: palette.mid
                    }

                    ListView {
                        id: snapshotsList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 10
                        model: backupsModel
                        leftMargin: 14
                        rightMargin: snapshotsScrollBar.visible ? snapshotsScrollBar.width + 20 : 14
                        topMargin: 14
                        bottomMargin: 14
                        ScrollBar.vertical: ScrollBar {
                            id: snapshotsScrollBar
                            policy: ScrollBar.AsNeeded
                            visible: snapshotsList.contentHeight > snapshotsList.height
                        }

                        delegate: ItemDelegate {
                            required property var modelData
                            width: snapshotsList.width - snapshotsList.leftMargin - snapshotsList.rightMargin
                            height: 104
                            padding: 0

                            background: Rectangle {
                                radius: 12
                                color: modelData.active ? palette.highlight : palette.window
                                border.color: modelData.active ? palette.highlight : palette.mid
                                border.width: modelData.active ? 2 : 1
                            }

                            contentItem: ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 14
                                spacing: 6

                                Label {
                                    Layout.fillWidth: true
                                    text: window.localizedSnapshotLabel(modelData.savedAtMs, modelData.snapshotLabel)
                                    elide: Text.ElideRight
                                    font.pixelSize: 17
                                    font.weight: Font.DemiBold
                                    color: modelData.active ? palette.highlightedText : palette.windowText
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.fileName
                                    elide: Text.ElideMiddle
                                    color: modelData.active ? palette.highlightedText : palette.placeholderText
                                }

                                RowLayout {
                                    spacing: 8

                                    Rectangle {
                                        radius: 7
                                        color: "transparent"
                                        border.color: modelData.active ? palette.highlightedText : palette.mid
                                        implicitWidth: snapshotSpaces.implicitWidth + 14
                                        implicitHeight: snapshotSpaces.implicitHeight + 8

                                        Label {
                                            id: snapshotSpaces
                                            anchors.centerIn: parent
                                            text: modelData.collections + " spaces"
                                            color: modelData.active ? palette.highlightedText : palette.windowText
                                        }
                                    }

                                    Rectangle {
                                        radius: 7
                                        color: "transparent"
                                        border.color: modelData.active ? palette.highlightedText : palette.mid
                                        implicitWidth: snapshotTabs.implicitWidth + 14
                                        implicitHeight: snapshotTabs.implicitHeight + 8

                                        Label {
                                            id: snapshotTabs
                                            anchors.centerIn: parent
                                            text: modelData.tabs + " tabs"
                                            color: modelData.active ? palette.highlightedText : palette.windowText
                                        }
                                    }
                                }
                            }

                            onClicked: if (backendRef) backendRef.select_backup(modelData.index)
                        }
                    }
                }
            }

            Frame {
                SplitView.fillWidth: true
                padding: 0

                background: Rectangle {
                    radius: 14
                    color: palette.base
                    border.color: palette.mid
                }

                contentItem: ColumnLayout {
                    spacing: 0

                    Item {
                        Layout.fillWidth: true
                        implicitHeight: detailsHeader.implicitHeight + 36

                        ColumnLayout {
                            id: detailsHeader
                            anchors.fill: parent
                            anchors.margins: 18
                            spacing: 12

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 16

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 4

                                    Label {
                                        Layout.fillWidth: true
                                        text: activeBackup.fileName ? window.localizedSnapshotLabel(activeBackup.savedAtMs, activeBackup.snapshotLabel) : "Select a snapshot"
                                        elide: Text.ElideMiddle
                                        font.pixelSize: 24
                                        font.weight: Font.DemiBold
                                        color: palette.windowText
                                    }

                                    Label {
                                        Layout.fillWidth: true
                                        text: activeBackup.fileName ? activeBackup.fileName : "Choose a snapshot from the left to inspect its spaces and tabs."
                                        elide: Text.ElideMiddle
                                        color: palette.placeholderText
                                    }
                                }

                                CheckBox {
                                    id: launchAfterRestore
                                    text: "Launch Zen after restore"
                                    checked: backendRef ? backendRef.launch_after_restore : false
                                    enabled: !!backendRef
                                    onToggled: if (backendRef) backendRef.set_launch_after_restore(checked)
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                visible: !!activeBackup.fileName

                                Repeater {
                                    model: [
                                        { "label": "Tabs", "value": activeBackup.totalTabs || 0 },
                                        { "label": "Selected", "value": activeBackup.selectedTabs || 0 },
                                        { "label": "Spaces", "value": activeBackup.collections ? activeBackup.collections.length : 0 }
                                    ]

                                    delegate: Rectangle {
                                        required property var modelData
                                        radius: 10
                                        color: palette.window
                                        border.color: palette.mid
                                        Layout.preferredWidth: 140
                                        implicitHeight: statColumn.implicitHeight + 20

                                        ColumnLayout {
                                            id: statColumn
                                            anchors.centerIn: parent
                                            spacing: 3

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
                                                font.pixelSize: 22
                                                font.weight: Font.DemiBold
                                            }
                                        }
                                    }
                                }

                                Item {
                                    Layout.fillWidth: true
                                }

                                Button {
                                    text: "Restore Full Backup"
                                    enabled: !!backendRef && !!activeBackup.fileName
                                    onClicked: fullRestoreDialog.open()
                                }

                                Button {
                                    text: "Restore Selected"
                                    highlighted: true
                                    enabled: !!backendRef && !!activeBackup.selectedTabs && activeBackup.selectedTabs > 0
                                    onClicked: selectiveRestoreDialog.open()
                                }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: palette.mid
                    }

                    ScrollView {
                        id: detailsScroll
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        padding: 14
                        clip: true

                        Item {
                            width: detailsScroll.availableWidth
                            implicitHeight: detailsColumn.implicitHeight

                            ColumnLayout {
                                id: detailsColumn
                                width: parent.width
                                spacing: 14

                                Rectangle {
                                    Layout.fillWidth: true
                                    visible: !activeBackup.fileName
                                    radius: 12
                                    color: palette.window
                                    border.color: palette.mid
                                    implicitHeight: emptyStateColumn.implicitHeight + 40

                                    ColumnLayout {
                                        id: emptyStateColumn
                                        anchors.centerIn: parent
                                        spacing: 8

                                        Label {
                                            text: "No snapshot selected"
                                            font.pixelSize: 20
                                            font.weight: Font.DemiBold
                                            horizontalAlignment: Text.AlignHCenter
                                            color: palette.windowText
                                        }

                                        Label {
                                            text: "Pick a snapshot from the left to review its spaces, tabs, and restore options."
                                            horizontalAlignment: Text.AlignHCenter
                                            wrapMode: Text.WordWrap
                                            color: palette.placeholderText
                                        }
                                    }
                                }

                                Repeater {
                                    model: activeBackup.collections ? activeBackup.collections : []

                                    delegate: Rectangle {
                                        required property var modelData
                                        property int collectionIndex: modelData.index
                                        Layout.fillWidth: true
                                        radius: 12
                                        color: palette.window
                                        border.color: palette.mid
                                        implicitHeight: collectionContent.implicitHeight + 28

                                        ColumnLayout {
                                            id: collectionContent
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 12

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 2

                                                    Label {
                                                        text: modelData.title
                                                        font.pixelSize: 18
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
                                                    onClicked: if (backendRef) backendRef.toggle_collection(modelData.index, modelData.selectedCount !== modelData.tabCount)
                                                }
                                            }

                                            Repeater {
                                                model: modelData.tabs

                                                delegate: Rectangle {
                                                    required property var modelData
                                                    Layout.fillWidth: true
                                                    radius: 10
                                                    color: "transparent"
                                                    border.color: modelData.selected ? palette.highlight : palette.mid
                                                    border.width: modelData.selected ? 2 : 1
                                                    implicitHeight: tabContent.implicitHeight + 20

                                                    RowLayout {
                                                        id: tabContent
                                                        anchors.fill: parent
                                                        anchors.margins: 10
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
                                                            spacing: 4

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

                                                                Rectangle {
                                                                    visible: window.tabBadgeText(modelData).length > 0
                                                                    radius: 7
                                                                    color: palette.base
                                                                    border.color: palette.mid
                                                                    implicitWidth: badgeLabel.implicitWidth + 16
                                                                    implicitHeight: badgeLabel.implicitHeight + 10

                                                                    Label {
                                                                        id: badgeLabel
                                                                        anchors.centerIn: parent
                                                                        text: window.tabBadgeText(modelData)
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
                                        }
                                    }
                                }

                                Item {
                                    Layout.fillHeight: true
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
                text: "Version 0.4"
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
