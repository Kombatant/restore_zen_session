import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

ApplicationWindow {
    id: window
    width: 1440
    height: 920
    minimumWidth: 1180
    minimumHeight: 760
    visible: true
    title: "Zen Session Restore"
    color: outerBackground
    flags: Qt.Window | Qt.FramelessWindowHint

    property var backupsModel: []
    property var activeBackup: ({})
    property var backendRef: typeof backend !== "undefined" ? backend : null
    property bool syncInFlight: false
    property string repositoryReadmeUrl: "https://github.com/Kombatant/restore_zen_session#readme"
    property int currentSection: 0
    property int kdeBaseFontPx: Qt.application.font.pixelSize > 0 ? Qt.application.font.pixelSize : 13
    property int kdeTinyFontPx: Math.max(10, kdeBaseFontPx - 3)
    property int kdeSmallFontPx: Math.max(11, kdeBaseFontPx - 1)
    property int kdeBodyFontPx: kdeBaseFontPx
    property int kdeUiFontPx: kdeBaseFontPx + 1
    property int kdeSidebarDateFontPx: kdeBaseFontPx + 2
    property int kdeSectionFontPx: kdeBaseFontPx + 4
    property int kdeTitleFontPx: kdeBaseFontPx + 11

    readonly property bool darkTheme: (palette.window.r * 0.299 + palette.window.g * 0.587 + palette.window.b * 0.114) < 0.5
    readonly property color textOnAccent: palette.highlightedText
    readonly property color warningBase: Qt.rgba(0.82, 0.58, 0.22, 1.0)
    readonly property color criticalBase: Qt.rgba(0.80, 0.34, 0.39, 1.0)
    readonly property color successBase: Qt.rgba(0.24, 0.66, 0.48, 1.0)

    property color outerBackground: palette.window
    property color shellBackground: blend(palette.window, palette.base, darkTheme ? 0.22 : 0.06)
    property color titlebarBackground: blend(palette.window, palette.button, darkTheme ? 0.28 : 0.08)
    property color sidebarBackground: blend(shellBackground, palette.base, darkTheme ? 0.22 : 0.05)
    property color surfaceBackground: palette.base
    property color cardBackground: blend(palette.base, palette.button, darkTheme ? 0.18 : 0.04)
    property color subtleCardBackground: blend(palette.alternateBase, palette.window, darkTheme ? 0.08 : 0.03)
    property color rowBackground: blend(subtleCardBackground, palette.windowText, darkTheme ? 0.05 : 0.02)
    property color hoverBackground: blend(cardBackground, accentBlue, darkTheme ? 0.14 : 0.08)
    property color frameColor: blend(palette.mid, palette.windowText, darkTheme ? 0.08 : 0.04)
    property color textPrimary: palette.windowText
    property color textMuted: blend(textPrimary, outerBackground, darkTheme ? 0.36 : 0.5)
    property color textFaint: blend(textPrimary, outerBackground, darkTheme ? 0.58 : 0.7)
    property color accentBlue: palette.highlight
    property color accentBlueSoft: blend(surfaceBackground, accentBlue, darkTheme ? 0.25 : 0.12)
    property color accentBlueFaint: blend(surfaceBackground, accentBlue, darkTheme ? 0.14 : 0.07)
    property color accentBlueBorder: blend(accentBlue, textPrimary, darkTheme ? 0.14 : 0.08)
    property color warningFill: blend(surfaceBackground, warningBase, darkTheme ? 0.22 : 0.12)
    property color warningBorder: blend(warningBase, textPrimary, darkTheme ? 0.12 : 0.06)
    property color warningText: blend(warningBase, textPrimary, darkTheme ? 0.32 : 0.18)
    property color criticalFill: blend(surfaceBackground, criticalBase, darkTheme ? 0.2 : 0.11)
    property color criticalBorder: blend(criticalBase, textPrimary, darkTheme ? 0.12 : 0.06)
    property color criticalText: blend(criticalBase, textPrimary, darkTheme ? 0.3 : 0.16)
    property color successText: blend(successBase, textPrimary, darkTheme ? 0.32 : 0.18)

    function blend(baseColor, mixColor, amount) {
        const clampedAmount = Math.max(0, Math.min(1, amount))
        return Qt.rgba(
            baseColor.r + (mixColor.r - baseColor.r) * clampedAmount,
            baseColor.g + (mixColor.g - baseColor.g) * clampedAmount,
            baseColor.b + (mixColor.b - baseColor.b) * clampedAmount,
            baseColor.a + (mixColor.a - baseColor.a) * clampedAmount
        )
    }

    function withAlpha(sourceColor, alphaValue) {
        return Qt.rgba(sourceColor.r, sourceColor.g, sourceColor.b, alphaValue)
    }

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

    function localizedSnapshotLabel(savedAtMs, fallbackLabel) {
        if (savedAtMs === undefined || savedAtMs === null)
            return fallbackLabel

        const snapshotDate = new Date(savedAtMs)
        if (isNaN(snapshotDate.getTime()))
            return fallbackLabel

        return Qt.locale().toString(snapshotDate, Locale.ShortFormat)
    }

    function restorableTabsInCollection(collection) {
        if (!collection || !collection.tabs)
            return 0

        let total = 0
        for (let i = 0; i < collection.tabs.length; ++i) {
            if (collection.tabs[i].restorable)
                total += 1
        }
        return total
    }

    function restorableTabsInBackup() {
        if (!activeBackup.collections)
            return 0

        let total = 0
        for (let i = 0; i < activeBackup.collections.length; ++i)
            total += restorableTabsInCollection(activeBackup.collections[i])
        return total
    }

    function toggleAllInActiveBackup() {
        if (!backendRef || !activeBackup.collections)
            return

        const totalRestorable = restorableTabsInBackup()
        const shouldSelect = !(totalRestorable > 0 && activeBackup.selectedTabs >= totalRestorable)
        for (let i = 0; i < activeBackup.collections.length; ++i)
            backendRef.toggle_collection(activeBackup.collections[i].index, shouldSelect)
    }

    function syncFooterLabel() {
        if (!backendRef)
            return "\u25cf GOOGLE SYNC UNAVAILABLE"
        if (!backendRef.google_oauth_ready)
            return "\u25cf GOOGLE SYNC NOT CONFIGURED"
        if (!backendRef.google_auth_connected)
            return "\u25cf GOOGLE SYNC DISCONNECTED"
        return "\u25cf CONNECTED TO GOOGLE SYNC"
    }

    function syncFooterColor() {
        if (!backendRef)
            return textFaint
        if (!backendRef.google_oauth_ready)
            return warningText
        if (!backendRef.google_auth_connected)
            return criticalText
        return successText
    }

    function hasMultipleCollections() {
        return !!activeBackup.collections && activeBackup.collections.length > 1
    }

    function statusChipLabel() {
        if (!backendRef)
            return "Offline"
        return backendRef.zen_running ? "Profile In Use" : "Profile Available"
    }

    function statusChipFill() {
        if (!backendRef)
            return subtleCardBackground
        return backendRef.zen_running ? criticalFill : surfaceBackground
    }

    function statusChipBorder() {
        if (!backendRef)
            return frameColor
        return backendRef.zen_running ? criticalBorder : frameColor
    }

    function statusChipColor() {
        if (!backendRef)
            return textMuted
        return backendRef.zen_running ? criticalText : textPrimary
    }

    function tabAccentColor(tab) {
        if (tab && !tab.restorable)
            return criticalText
        if (tab && tab.essential)
            return warningText
        if (tab && tab.pinned)
            return accentBlue
        return textMuted
    }

    function tabAccentFillColor(tab) {
        if (tab && !tab.restorable)
            return criticalFill
        if (tab && tab.essential)
            return warningFill
        if (tab && tab.pinned)
            return accentBlueFaint
        return rowBackground
    }

    function tabMonogram(tab) {
        if (tab && tab.url) {
            let host = tab.url.replace(/^https?:\/\//, "").split("/")[0]
            let parts = host.split(".").filter(function(part) { return part.length > 0 && part !== "www" })
            if (parts.length > 0) {
                let base = parts.length > 1 ? parts[parts.length - 2] : parts[0]
                return base.substring(0, Math.min(2, base.length)).toUpperCase()
            }
        }

        const source = tab && tab.title ? tab.title : "ZS"
        const words = source.split(/[\s\-_:.|/]+/).filter(function(part) { return part.length > 0 })
        if (words.length >= 2)
            return (words[0][0] + words[1][0]).toUpperCase()
        return source.substring(0, Math.min(2, source.length)).toUpperCase()
    }

    function tabGlyph(tab) {
        const key = (((tab && tab.url) ? tab.url : "") + " " + ((tab && tab.title) ? tab.title : "")).toLowerCase()

        if (key.indexOf("weather") !== -1)
            return "\u2601"
        if (key.indexOf("outlook") !== -1 || key.indexOf("mail") !== -1 || key.indexOf("inbox") !== -1)
            return "\u2709"
        if (key.indexOf("github") !== -1)
            return "\u2398"
        if (key.indexOf("google") !== -1 || key.indexOf("search") !== -1)
            return "\u2315"
        if (key.indexOf("docker") !== -1 || key.indexOf("code") !== -1)
            return "<>"
        if (key.indexOf("terminal") !== -1 || key.indexOf("shell") !== -1)
            return "\u2332"
        return "\u25e7"
    }

    component ChromeButton: Button {
        id: control
        property bool danger: false

        implicitWidth: 28
        implicitHeight: 28
        padding: 0
        hoverEnabled: true

        background: Rectangle {
            radius: 4
            color: {
                if (!control.enabled)
                    return "transparent"
                if (control.danger)
                    return control.down ? blend(criticalFill, criticalText, 0.2) : (control.hovered ? criticalFill : "transparent")
                return control.down ? blend(titlebarBackground, textPrimary, 0.1) : (control.hovered ? hoverBackground : "transparent")
            }
            border.width: control.hovered ? 1 : 0
            border.color: control.danger ? criticalBorder : frameColor
        }

        contentItem: Label {
            text: control.text
            color: control.danger ? criticalText : textMuted
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            font.pixelSize: window.kdeBodyFontPx
            font.weight: Font.Medium
        }
    }

    component ActionButton: Button {
        id: control
        property bool primary: false
        property bool compact: false
        property bool warning: false

        hoverEnabled: true
        leftPadding: compact ? 12 : 16
        rightPadding: compact ? 12 : 16
        topPadding: compact ? 7 : 10
        bottomPadding: compact ? 7 : 10

        background: Rectangle {
            radius: 4
            opacity: control.enabled ? 1.0 : 0.5
            color: {
                if (control.primary)
                    return control.down ? blend(accentBlue, textPrimary, 0.14) : accentBlue
                if (control.warning)
                    return control.down ? blend(warningFill, warningText, 0.14) : warningFill
                return control.down ? blend(cardBackground, textPrimary, 0.08) : (control.hovered ? hoverBackground : cardBackground)
            }
            border.width: 1
            border.color: {
                if (control.primary)
                    return accentBlueBorder
                if (control.warning)
                    return warningBorder
                return frameColor
            }
        }

        contentItem: Label {
            text: control.text
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: control.primary ? textOnAccent : (control.warning ? warningText : textPrimary)
            font.pixelSize: control.compact ? window.kdeSmallFontPx : window.kdeBodyFontPx
            font.weight: Font.Medium
        }
    }

    component AccentCheckBox: CheckBox {
        id: control
        spacing: 8
        hoverEnabled: true

        indicator: Rectangle {
            implicitWidth: 16
            implicitHeight: 16
            radius: 3
            color: control.checked ? accentBlue : "transparent"
            border.width: 1
            border.color: control.enabled ? (control.checked ? accentBlueBorder : blend(frameColor, textMuted, 0.35)) : frameColor

            Label {
                anchors.centerIn: parent
                visible: control.checked
                text: "\u2713"
                color: textOnAccent
                font.pixelSize: 11
                font.weight: Font.DemiBold
            }
        }

        contentItem: Label {
            text: control.text
            leftPadding: control.indicator.width + control.spacing
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            color: control.enabled ? textPrimary : textFaint
            font.pixelSize: window.kdeBodyFontPx
        }
    }

    component AccentSpinBox: SpinBox {
        id: control
        implicitWidth: 88
        implicitHeight: 32
        editable: false

        contentItem: TextInput {
            text: control.displayText
            readOnly: true
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: textPrimary
            font.pixelSize: window.kdeBodyFontPx
            selectedTextColor: textPrimary
            selectionColor: accentBlueSoft
        }

        up.indicator: Rectangle {
            implicitWidth: 24
            implicitHeight: 30
            x: control.width - width - 1
            y: 1
            radius: 4
            color: control.up.pressed ? hoverBackground : "transparent"

            Label {
                anchors.centerIn: parent
                text: "+"
                color: textMuted
                font.pixelSize: window.kdeUiFontPx
                font.weight: Font.DemiBold
            }
        }

        down.indicator: Rectangle {
            implicitWidth: 24
            implicitHeight: 30
            x: 1
            y: 1
            radius: 4
            color: control.down.pressed ? hoverBackground : "transparent"

            Label {
                anchors.centerIn: parent
                text: "-"
                color: textMuted
                font.pixelSize: window.kdeSectionFontPx
                font.weight: Font.DemiBold
            }
        }

        background: Rectangle {
            radius: 4
            color: titlebarBackground
            border.width: 1
            border.color: frameColor
        }
    }

    component MetaChip: Rectangle {
        id: chip
        property string label: ""
        property color fillColor: surfaceBackground
        property color strokeColor: frameColor
        property color labelColor: textMuted

        implicitHeight: 20
        implicitWidth: chipLabel.implicitWidth + 14
        radius: 10
        color: fillColor
        border.width: 1
        border.color: strokeColor

        Label {
            id: chipLabel
            anchors.centerIn: parent
            text: chip.label
            color: chip.labelColor
            font.pixelSize: window.kdeTinyFontPx
            font.weight: Font.DemiBold
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

    background: Rectangle {
        color: outerBackground
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 0
        radius: 0
        clip: true
        color: shellBackground
        border.width: 0
        border.color: "transparent"

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 40
                color: titlebarBackground
                border.width: 1
                border.color: "transparent"

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton
                    onPressed: function(mouse) {
                        if (mouse.button === Qt.LeftButton)
                            window.startSystemMove()
                    }
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 8
                    spacing: 10

                    RowLayout {
                        spacing: 8

                        Item {
                            width: 18
                            height: 18

                            Label {
                                anchors.left: parent.left
                                anchors.top: parent.top
                                text: "\u2726"
                                color: accentBlue
                                font.pixelSize: 10
                            }

                            Label {
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                text: "\u2726"
                                color: accentBlueBorder
                                font.pixelSize: 8
                            }
                        }

                        Label {
                            text: "Zen Session Restore"
                            color: textPrimary
                            font.pixelSize: window.kdeSmallFontPx
                            font.weight: Font.Medium
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        spacing: 4

                        ChromeButton {
                            text: "\u21bb"
                            enabled: !!backendRef
                            onClicked: if (backendRef) backendRef.refresh()
                        }

                        ChromeButton {
                            text: "🗀"
                            enabled: !!backendRef
                            onClicked: profileFolderDialog.open()
                        }
                    }

                    Rectangle {
                        width: 1
                        height: 18
                        color: frameColor
                        opacity: 0.85
                    }

                    RowLayout {
                        spacing: 4

                        ChromeButton {
                            text: "?"
                            onClicked: aboutDialog.open()
                        }

                        ChromeButton {
                            text: "\u2500"
                            onClicked: window.showMinimized()
                        }

                        ChromeButton {
                            text: "\u25a1"
                            onClicked: {
                                if (window.visibility === Window.Maximized)
                                    window.showNormal()
                                else
                                    window.showMaximized()
                            }
                        }

                        ChromeButton {
                            text: "\u2715"
                            danger: true
                            onClicked: window.close()
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                Rectangle {
                    Layout.preferredWidth: 330
                    Layout.fillHeight: true
                    color: sidebarBackground
                    border.width: 1
                    border.color: "transparent"

                    Rectangle {
                        anchors.top: parent.top
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        width: 1
                        color: frameColor
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        Item {
                            Layout.fillWidth: true
                            implicitHeight: 70

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 16
                                spacing: 6

                                Label {
                                    text: "SNAPSHOT SESSIONS"
                                    color: textFaint
                                    font.pixelSize: window.kdeSmallFontPx
                                    font.weight: Font.DemiBold
                                    font.letterSpacing: 1.3
                                }

                            }
                        }

                        ListView {
                            id: snapshotList
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            spacing: 8
                            model: backupsModel
                            leftMargin: 10
                            rightMargin: snapshotScrollBar.visible ? snapshotScrollBar.width + 12 : 10
                            topMargin: 6
                            bottomMargin: 10

                            ScrollBar.vertical: ScrollBar {
                                id: snapshotScrollBar
                                policy: ScrollBar.AsNeeded
                            }

                            delegate: Rectangle {
                                id: snapshotCard
                                required property var modelData
                                width: snapshotList.width - snapshotList.leftMargin - snapshotList.rightMargin
                                height: 80
                                radius: 4
                                color: modelData.active ? accentBlueSoft : (snapshotHover.containsMouse ? subtleCardBackground : "transparent")
                                border.width: 1
                                border.color: modelData.active ? accentBlueBorder : "transparent"

                                MouseArea {
                                    id: snapshotHover
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: if (backendRef) backendRef.select_backup(snapshotCard.modelData.index)
                                }

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 12
                                    spacing: 3

                                    Label {
                                        Layout.fillWidth: true
                                        text: window.localizedSnapshotLabel(snapshotCard.modelData.savedAtMs, snapshotCard.modelData.snapshotLabel)
                                        elide: Text.ElideRight
                                        color: snapshotCard.modelData.active ? accentBlue : textPrimary
                                        font.pixelSize: window.kdeSidebarDateFontPx
                                        font.weight: Font.Medium
                                    }

                                    Label {
                                        Layout.fillWidth: true
                                        text: (snapshotCard.modelData.collections + " spaces \u2022 " + snapshotCard.modelData.tabs + " tabs").toUpperCase()
                                        elide: Text.ElideRight
                                        color: snapshotCard.modelData.active ? accentBlueBorder : textFaint
                                        font.pixelSize: window.kdeTinyFontPx
                                        font.weight: Font.DemiBold
                                        font.letterSpacing: 0.7
                                    }
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: shellBackground

                    Rectangle {
                        anchors.top: parent.top
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        width: 360
                        color: "transparent"

                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0.0; color: withAlpha(accentBlue, 0.0) }
                            GradientStop { position: 1.0; color: withAlpha(accentBlue, darkTheme ? 0.2 : 0.12) }
                        }
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        Item {
                            Layout.fillWidth: true
                            implicitHeight: 104

                            Label {
                                anchors.left: parent.left
                                anchors.top: parent.top
                                anchors.leftMargin: 24
                                anchors.topMargin: 20
                                text: "Zen Session Restore"
                                color: textPrimary
                                font.pixelSize: window.kdeTitleFontPx
                                font.weight: Font.Normal
                            }

                            ActionButton {
                                id: restoreSelectedButton
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.rightMargin: 28
                                anchors.topMargin: 18
                                visible: currentSection === 0
                                text: "\u2299 Restore Selected"
                                primary: true
                                enabled: !!backendRef && !!activeBackup.selectedTabs && activeBackup.selectedTabs > 0
                                leftPadding: 24
                                rightPadding: 24
                                topPadding: 13
                                bottomPadding: 13
                                onClicked: selectiveRestoreDialog.open()

                                TapHandler {
                                    acceptedButtons: Qt.RightButton
                                    onTapped: restoreMoreMenu.open()
                                }
                            }
                        }

                        Item {
                            Layout.fillWidth: true
                            implicitHeight: 50

                            Rectangle {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                anchors.leftMargin: 24
                                anchors.rightMargin: 24
                                height: 1
                                color: frameColor
                            }

                            RowLayout {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                anchors.leftMargin: 24
                                anchors.rightMargin: 24
                                spacing: 22

                                Repeater {
                                    model: [
                                        { "label": "Snapshot", "index": 0 },
                                        { "label": "Google Sync", "index": 1 }
                                    ]

                                    delegate: Item {
                                        required property var modelData
                                        implicitWidth: tabLabel.implicitWidth
                                        implicitHeight: 34

                                        MouseArea {
                                            anchors.fill: parent
                                            onClicked: currentSection = modelData.index
                                        }

                                        Label {
                                            id: tabLabel
                                            anchors.left: parent.left
                                            anchors.bottom: parent.bottom
                                            anchors.bottomMargin: 8
                                            text: modelData.label
                                            color: currentSection === modelData.index ? accentBlue : textMuted
                                            font.pixelSize: window.kdeUiFontPx
                                            font.weight: Font.Medium
                                        }

                                        Rectangle {
                                            anchors.left: parent.left
                                            anchors.right: parent.right
                                            anchors.bottom: parent.bottom
                                            height: 2
                                            color: currentSection === modelData.index ? accentBlue : "transparent"
                                        }
                                    }
                                }

                                Item {
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        StackLayout {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            currentIndex: currentSection

                            Item {
                                Layout.fillWidth: true
                                Layout.fillHeight: true

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 24
                                    anchors.rightMargin: 28
                                    anchors.topMargin: 12
                                    anchors.bottomMargin: 18
                                    spacing: 16

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10

                                        Label {
                                            text: "Spaces"
                                            color: textPrimary
                                            font.pixelSize: window.kdeSectionFontPx
                                            font.weight: Font.Medium
                                        }

                                        Label {
                                            Layout.fillWidth: true
                                            text: activeBackup.collections
                                                ? activeBackup.collections.length + " spaces \u2022 " + (activeBackup.totalTabs || 0) + " tabs \u2022 " + (activeBackup.selectedTabs || 0) + " selected"
                                                : "No snapshot selected"
                                            color: textMuted
                                            font.pixelSize: window.kdeBodyFontPx
                                        }

                                        ActionButton {
                                            compact: true
                                            Layout.preferredWidth: restoreSelectedButton.width
                                            text: "Select / Deselect All"
                                            primary: restorableTabsInBackup() > 0 && activeBackup.selectedTabs >= restorableTabsInBackup()
                                            enabled: !!backendRef && !!activeBackup.collections && activeBackup.collections.length > 0
                                            onClicked: toggleAllInActiveBackup()
                                        }
                                    }

                                    ScrollView {
                                        id: snapshotScroll
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        clip: true

                                        Item {
                                            width: snapshotScroll.availableWidth
                                            implicitHeight: snapshotColumn.implicitHeight

                                            ColumnLayout {
                                                id: snapshotColumn
                                                width: parent.width
                                                spacing: 12

                                                Rectangle {
                                                    Layout.fillWidth: true
                                                    visible: !activeBackup.fileName
                                                    radius: 4
                                                    color: subtleCardBackground
                                                    border.width: 1
                                                    border.color: frameColor
                                                    implicitHeight: emptyStateContent.implicitHeight + 40

                                                    ColumnLayout {
                                                        id: emptyStateContent
                                                        anchors.centerIn: parent
                                                        spacing: 8

                                                        Label {
                                                            text: "No snapshot selected"
                                                            color: textPrimary
                                                            font.pixelSize: 20
                                                            font.weight: Font.Medium
                                                            horizontalAlignment: Text.AlignHCenter
                                                        }

                                                        Label {
                                                            text: "Pick a snapshot from the left rail to inspect its spaces, tabs, and restore options."
                                                            color: textMuted
                                                            wrapMode: Text.WordWrap
                                                            horizontalAlignment: Text.AlignHCenter
                                                        }
                                                    }
                                                }

                                                Repeater {
                                                    model: activeBackup.collections ? activeBackup.collections : []

                                                    delegate: Item {
                                                        id: collectionCard
                                                        required property var modelData
                                                        property int collectionIndex: modelData.index
                                                        property int selectableTabCount: window.restorableTabsInCollection(modelData)
                                                        Layout.fillWidth: true
                                                        implicitHeight: collectionBody.implicitHeight

                                                        ColumnLayout {
                                                            id: collectionBody
                                                            anchors.left: parent.left
                                                            anchors.right: parent.right
                                                            spacing: 10

                                                            RowLayout {
                                                                Layout.fillWidth: true
                                                                spacing: 10
                                                                visible: false

                                                                ColumnLayout {
                                                                    Layout.fillWidth: true
                                                                    spacing: 2

                                                                    Label {
                                                                        text: collectionCard.modelData.title
                                                                        color: textPrimary
                                                                        font.pixelSize: 14
                                                                        font.weight: Font.Medium
                                                                    }

                                                                    RowLayout {
                                                                        Layout.fillWidth: true
                                                                        spacing: 8

                                                                        Label {
                                                                            text: collectionCard.modelData.selectedCount + " selected \u2022 " + collectionCard.modelData.tabCount + " tabs"
                                                                            color: textMuted
                                                                            font.pixelSize: 12
                                                                        }

                                                                        Label {
                                                                            visible: !!collectionCard.modelData.workspaceId
                                                                            text: collectionCard.modelData.workspaceId ? "Workspace " + collectionCard.modelData.workspaceId : ""
                                                                            color: textFaint
                                                                            font.pixelSize: 11
                                                                        }
                                                                    }
                                                                }

                                                                ActionButton {
                                                                    compact: true
                                                                    text: collectionCard.selectableTabCount > 0 && collectionCard.modelData.selectedCount >= collectionCard.selectableTabCount ? "Deselect All" : "Select All"
                                                                    enabled: !!backendRef && collectionCard.selectableTabCount > 0
                                                                    visible: false
                                                                    onClicked: if (backendRef) backendRef.toggle_collection(collectionCard.modelData.index, !(collectionCard.selectableTabCount > 0 && collectionCard.modelData.selectedCount >= collectionCard.selectableTabCount))
                                                                }
                                                            }

                                                            Repeater {
                                                                model: collectionCard.modelData.tabs

                                                                delegate: Rectangle {
                                                                    required property var modelData
                                                                    Layout.fillWidth: true
                                                                    radius: 4
                                                                    color: modelData.selected ? accentBlueSoft : rowBackground
                                                                    border.width: 1
                                                                    border.color: modelData.selected ? accentBlueBorder : frameColor
                                                                    implicitHeight: tabRowLayout.implicitHeight + 18

                                                                    RowLayout {
                                                                        id: tabRowLayout
                                                                        anchors.fill: parent
                                                                        anchors.margins: 12
                                                                        spacing: 12

                                                                        AccentCheckBox {
                                                                            Layout.alignment: Qt.AlignTop
                                                                            checked: modelData.selected
                                                                            enabled: modelData.restorable
                                                                            onToggled: if (backendRef) backendRef.toggle_tab(collectionCard.collectionIndex, modelData.index, checked)
                                                                        }

                                                                        Rectangle {
                                                                            Layout.alignment: Qt.AlignTop
                                                                            width: 38
                                                                            height: 38
                                                                            radius: 4
                                                                            color: window.tabAccentFillColor(modelData)
                                                                            border.width: 0

                                                                            Label {
                                                                                anchors.centerIn: parent
                                                                                text: window.tabGlyph(modelData)
                                                                                color: modelData.selected ? accentBlue : window.tabAccentColor(modelData)
                                                                                font.pixelSize: window.tabGlyph(modelData) === "<>" ? 13 : 16
                                                                                font.weight: Font.DemiBold
                                                                            }
                                                                        }

                                                                        ColumnLayout {
                                                                            Layout.fillWidth: true
                                                                            Layout.alignment: Qt.AlignTop
                                                                            spacing: 4

                                                                            RowLayout {
                                                                                Layout.fillWidth: true
                                                                                spacing: 8

                                                                                Label {
                                                                                    Layout.fillWidth: true
                                                                                    text: modelData.title
                                                                                    elide: Text.ElideRight
                                                                                    maximumLineCount: 1
                                                                                    color: textPrimary
                                                                                    font.pixelSize: window.kdeUiFontPx
                                                                                    font.weight: Font.Medium
                                                                                }

                                                                                MetaChip {
                                                                                    visible: !!modelData.pinned
                                                                                    label: "Pinned"
                                                                                    fillColor: accentBlueFaint
                                                                                    strokeColor: "transparent"
                                                                                    labelColor: modelData.selected ? accentBlue : textMuted
                                                                                }

                                                                                MetaChip {
                                                                                    visible: !!modelData.essential
                                                                                    label: "Essential"
                                                                                    fillColor: warningFill
                                                                                    strokeColor: warningBorder
                                                                                    labelColor: warningText
                                                                                }

                                                                                MetaChip {
                                                                                    visible: !modelData.restorable
                                                                                    label: "Unsupported"
                                                                                    fillColor: criticalFill
                                                                                    strokeColor: criticalBorder
                                                                                    labelColor: criticalText
                                                                                }
                                                                            }

                                                                            Label {
                                                                                Layout.fillWidth: true
                                                                                text: modelData.url ? modelData.url : "Unsupported or missing URL"
                                                                                elide: Text.ElideRight
                                                                                maximumLineCount: 1
                                                                                color: modelData.selected ? accentBlue : textMuted
                                                                                font.pixelSize: window.kdeBodyFontPx
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

                            Item {
                                Layout.fillWidth: true
                                Layout.fillHeight: true

                                ScrollView {
                                    anchors.fill: parent
                                    clip: true

                                    Item {
                                        width: parent.width
                                        implicitHeight: syncColumn.implicitHeight

                                        ColumnLayout {
                                            id: syncColumn
                                            width: parent.width
                                            anchors.leftMargin: 0
                                            anchors.rightMargin: 0
                                            spacing: 16

                                            Rectangle {
                                                Layout.fillWidth: true
                                                Layout.leftMargin: 24
                                                Layout.rightMargin: 24
                                                Layout.topMargin: 18
                                                radius: 4
                                                color: subtleCardBackground
                                                border.width: 1
                                                border.color: frameColor
                                                implicitHeight: syncSummary.implicitHeight + 28

                                                ColumnLayout {
                                                    id: syncSummary
                                                    anchors.fill: parent
                                                    anchors.margins: 14
                                                    spacing: 8

                                                    Label {
                                                        text: "Google Drive / Google One"
                                                        color: textPrimary
                                                        font.pixelSize: 20
                                                        font.weight: Font.Medium
                                                    }

                                                    Label {
                                                        Layout.fillWidth: true
                                                        wrapMode: Text.WordWrap
                                                        text: backendRef ? backendRef.cloud_sync_status_text : "Configure Google Drive sync for zen-sessions-backup."
                                                        color: textMuted
                                                    }
                                                }
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                Layout.leftMargin: 24
                                                Layout.rightMargin: 24
                                                visible: !backendRef || !backendRef.google_oauth_ready
                                                radius: 4
                                                color: warningFill
                                                border.width: 1
                                                border.color: warningBorder
                                                implicitHeight: googleWarning.implicitHeight + 24

                                                Label {
                                                    id: googleWarning
                                                    anchors.fill: parent
                                                    anchors.margins: 12
                                                    wrapMode: Text.WordWrap
                                                    textFormat: Text.RichText
                                                    color: warningText
                                                    linkColor: accentBlue
                                                    text: "Cloud Sync is disabled because <b>google.json</b> was not found next to the app. Add your Google Client ID and Client Secret in that file, then reopen the app. See the GitHub README for setup details: <a href=\"" + repositoryReadmeUrl + "\">" + repositoryReadmeUrl + "</a>"
                                                    onLinkActivated: function(link) { Qt.openUrlExternally(link) }
                                                }
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                Layout.leftMargin: 24
                                                Layout.rightMargin: 24
                                                radius: 4
                                                color: subtleCardBackground
                                                border.width: 1
                                                border.color: frameColor
                                                implicitHeight: syncControls.implicitHeight + 28

                                                ColumnLayout {
                                                    id: syncControls
                                                    anchors.fill: parent
                                                    anchors.margins: 14
                                                    spacing: 14

                                                    AccentCheckBox {
                                                        text: "Sync backup folder"
                                                        checked: backendRef ? backendRef.cloud_sync_enabled : false
                                                        enabled: !!backendRef && backendRef.google_oauth_ready
                                                        onToggled: if (backendRef) backendRef.set_cloud_sync_enabled(checked)
                                                    }

                                                    ColumnLayout {
                                                        spacing: 10

                                                        Item {
                                                            Layout.preferredWidth: Math.min(syncControls.width, 280)
                                                            Layout.preferredHeight: syncActionsColumn.implicitHeight

                                                            ColumnLayout {
                                                                id: syncActionsColumn
                                                                anchors.fill: parent
                                                                spacing: 10

                                                                ActionButton {
                                                                    Layout.fillWidth: true
                                                                    text: "Sync Now"
                                                                    primary: true
                                                                    enabled: !!backendRef && backendRef.google_oauth_ready && backendRef.cloud_sync_enabled && backendRef.google_auth_connected && !syncInFlight && !backendRef.cloud_sync_in_progress
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

                                                                ActionButton {
                                                                    Layout.fillWidth: true
                                                                    text: backendRef && backendRef.google_auth_connected ? "Reconnect Google Drive" : "Connect Google Drive"
                                                                    enabled: !!backendRef && backendRef.google_oauth_ready
                                                                    onClicked: if (backendRef) backendRef.connect_google_drive()
                                                                }

                                                                ActionButton {
                                                                    Layout.fillWidth: true
                                                                    text: "Disconnect"
                                                                    warning: true
                                                                    enabled: !!backendRef && backendRef.google_oauth_ready && backendRef.google_auth_connected
                                                                    onClicked: if (backendRef) backendRef.disconnect_google_drive()
                                                                }
                                                            }
                                                        }
                                                    }

                                                    RowLayout {
                                                        spacing: 10

                                                        Label {
                                                            text: "Retention"
                                                            color: textPrimary
                                                        }

                                                        AccentSpinBox {
                                                            from: 1
                                                            to: 12
                                                            value: backendRef ? backendRef.retention_months : 3
                                                            enabled: !!backendRef && backendRef.google_oauth_ready
                                                            onValueModified: if (backendRef) backendRef.set_retention_months(value)
                                                        }

                                                        Label {
                                                            text: "months"
                                                            color: textMuted
                                                        }
                                                    }

                                                    Label {
                                                        Layout.fillWidth: true
                                                        wrapMode: Text.WordWrap
                                                        color: textPrimary
                                                        text: backendRef && backendRef.google_oauth_ready
                                                            ? (backendRef.google_auth_connected
                                                                ? "Google Drive is connected for this app."
                                                                : "Connect in your browser to grant Google Drive access.")
                                                            : "Google Drive sign-in is disabled until google.json is added beside the app."
                                                    }

                                                    Label {
                                                        Layout.fillWidth: true
                                                        wrapMode: Text.WordWrap
                                                        color: textMuted
                                                        text: "The app opens your browser for Google sign-in, stores the refresh token locally, and mirrors zen-sessions-backup into Google Drive under Backup/Zen."
                                                    }

                                                    ColumnLayout {
                                                        Layout.fillWidth: true
                                                        spacing: 8
                                                        visible: backendRef && backendRef.cloud_sync_in_progress

                                                        Rectangle {
                                                            id: syncProgressTrack
                                                            Layout.fillWidth: true
                                                            implicitHeight: 12
                                                            radius: 6
                                                            color: rowBackground
                                                            border.width: 1
                                                            border.color: accentBlueFaint
                                                            clip: true

                                                            property bool determinate: backendRef && backendRef.cloud_sync_progress_total > 1
                                                            property real progressRatio: {
                                                                if (!backendRef || backendRef.cloud_sync_progress_total <= 0)
                                                                    return 0
                                                                return Math.max(0, Math.min(1, backendRef.cloud_sync_progress_current / backendRef.cloud_sync_progress_total))
                                                            }

                                                            Rectangle {
                                                                id: syncProgressFill
                                                                visible: syncProgressTrack.determinate
                                                                anchors.left: parent.left
                                                                anchors.top: parent.top
                                                                anchors.bottom: parent.bottom
                                                                width: Math.max(18, syncProgressTrack.width * syncProgressTrack.progressRatio)
                                                                radius: 6
                                                                gradient: Gradient {
                                                                    orientation: Gradient.Horizontal
                                                                    GradientStop { position: 0.0; color: blend(accentBlue, textPrimary, 0.05) }
                                                                    GradientStop { position: 0.55; color: accentBlue }
                                                                    GradientStop { position: 1.0; color: accentBlueBorder }
                                                                }

                                                                Behavior on width {
                                                                    NumberAnimation {
                                                                        duration: 260
                                                                        easing.type: Easing.OutCubic
                                                                    }
                                                                }

                                                                Rectangle {
                                                                    id: syncProgressShimmer
                                                                    visible: syncProgressFill.visible
                                                                    width: Math.max(36, syncProgressFill.width * 0.32)
                                                                    anchors.top: parent.top
                                                                    anchors.bottom: parent.bottom
                                                                    radius: 6
                                                                    color: withAlpha(textOnAccent, 0.4)
                                                                    opacity: 0.22
                                                                    x: -width

                                                                    SequentialAnimation on x {
                                                                        running: syncProgressShimmer.visible
                                                                        loops: Animation.Infinite
                                                                        NumberAnimation {
                                                                            from: -syncProgressShimmer.width
                                                                            to: syncProgressFill.width
                                                                            duration: 1150
                                                                            easing.type: Easing.InOutQuad
                                                                        }
                                                                        PauseAnimation { duration: 220 }
                                                                    }
                                                                }
                                                            }

                                                            Rectangle {
                                                                id: syncProgressIndeterminate
                                                                visible: !syncProgressTrack.determinate
                                                                width: Math.max(84, syncProgressTrack.width * 0.22)
                                                                anchors.top: parent.top
                                                                anchors.bottom: parent.bottom
                                                                radius: 6
                                                                gradient: Gradient {
                                                                    orientation: Gradient.Horizontal
                                                                    GradientStop { position: 0.0; color: blend(accentBlue, surfaceBackground, 0.12) }
                                                                    GradientStop { position: 0.5; color: accentBlueBorder }
                                                                    GradientStop { position: 1.0; color: blend(accentBlue, surfaceBackground, 0.12) }
                                                                }
                                                                x: -width

                                                                SequentialAnimation on x {
                                                                    running: syncProgressIndeterminate.visible
                                                                    loops: Animation.Infinite
                                                                    NumberAnimation {
                                                                        from: -syncProgressIndeterminate.width
                                                                        to: syncProgressTrack.width
                                                                        duration: 980
                                                                        easing.type: Easing.InOutQuad
                                                                    }
                                                                    PauseAnimation { duration: 90 }
                                                                }
                                                            }
                                                        }

                                                        Label {
                                                            Layout.alignment: Qt.AlignRight
                                                            visible: backendRef && backendRef.cloud_sync_progress_total > 1
                                                            color: textMuted
                                                            font.pixelSize: window.kdeTinyFontPx
                                                            text: backendRef
                                                                ? Math.round(Math.max(0, Math.min(1, backendRef.cloud_sync_progress_current / Math.max(1, backendRef.cloud_sync_progress_total))) * 100) + "%"
                                                                : ""
                                                        }

                                                        Label {
                                                            Layout.fillWidth: true
                                                            wrapMode: Text.WordWrap
                                                            color: textPrimary
                                                            text: backendRef && backendRef.cloud_sync_progress_text.length > 0
                                                                ? backendRef.cloud_sync_progress_text
                                                                : "Syncing backups with Google Drive..."
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

                        Rectangle {
                            id: footerDivider
                            Layout.fillWidth: true
                            implicitHeight: 24
                            color: titlebarBackground

                            Rectangle {
                                anchors.top: parent.top
                                anchors.left: parent.left
                                anchors.right: parent.right
                                height: 1
                                color: frameColor
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 14
                                anchors.rightMargin: 14
                                spacing: 14

                                RowLayout {
                                    spacing: 8

                                    Label {
                                        text: "PROFILE:"
                                        color: textFaint
                                        font.pixelSize: 10
                                        font.weight: Font.DemiBold
                                        font.letterSpacing: 0.9
                                    }

                                    Label {
                                        text: backendRef && backendRef.profile_path.length > 0 ? backendRef.profile_path.toUpperCase() : "NO ZEN PROFILE LOADED"
                                        color: textMuted
                                        font.pixelSize: 10
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideMiddle
                                        Layout.preferredWidth: Math.max(260, window.width * 0.38)
                                    }
                                }

                                Item {
                                    Layout.fillWidth: true
                                }

                                Label {
                                    text: activeBackup.fileName ? (activeBackup.totalTabs || 0) + " TABS LOADED" : backupsModel.length + " SNAPSHOTS FOUND"
                                    color: textMuted
                                    font.pixelSize: 10
                                    font.weight: Font.DemiBold
                                    font.letterSpacing: 0.9
                                }

                                Label {
                                    text: syncFooterLabel()
                                    color: syncFooterColor()
                                    font.pixelSize: 10
                                    font.weight: Font.DemiBold
                                    font.letterSpacing: 0.9
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Menu {
        id: restoreMoreMenu

        Action {
            text: "Restore Full Backup"
            enabled: !!backendRef && !!activeBackup.fileName
            onTriggered: fullRestoreDialog.open()
        }

        Action {
            text: "Launch Zen After Restore"
            checkable: true
            checked: backendRef ? backendRef.launch_after_restore : false
            enabled: !!backendRef
            onTriggered: if (backendRef) backendRef.set_launch_after_restore(checked)
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
                text: "Version 0.5.0"
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
