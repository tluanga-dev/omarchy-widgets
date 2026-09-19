import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

// Camera icon for the bar with a popup menu of capture modes. Every capture
// goes through omarchy-capture-screenshot's `slurp` flow, which writes the
// PNG to disk and puts it on the clipboard in one go; the widget only points
// it at the configured folder. Right-click on the icon skips the menu and
// starts an area capture straight away.
Panel {
  id: root
  moduleName: "dev.screenshot"
  ipcTarget: "dev.screenshot"

  readonly property string directory: String(setting("directory", "~/Documents/Screenshots"))
  property bool cursorActive: false
  property int cursorIndex: 0

  readonly property var actions: [
    { icon: "󰩭", title: "Select area", subtitle: "Drag a region · copied + saved", command: "screenshot region" },
    { icon: "󰖯", title: "Window", subtitle: "Pick a window · copied + saved", command: "screenshot windows" },
    { icon: "󰍹", title: "Full screen", subtitle: "Whole screen · copied + saved", command: "screenshot fullscreen" },
    { icon: "󰊄", title: "Copy text", subtitle: "OCR a region to the clipboard", command: "text" },
    { icon: "󰉏", title: "Open folder", subtitle: "", command: "folder" }
  ]

  readonly property real openPanelIndicatorWidth: 0

  // Close first so the popup is gone before the screen freeze and the region
  // picker take over; the delay covers the popup's fade-out.
  function capture(command) {
    close()
    var dir = Util.shellQuote(directory.replace(/^~(?=\/|$)/, Quickshell.env("HOME")))
    var script
    if (command === "folder") script = "xdg-open " + dir
    else if (command === "text") script = "sleep 0.3; omarchy-capture-text"
    else script = "sleep 0.3; OMARCHY_SCREENSHOT_DIR=" + dir + " omarchy-capture-" + command + " slurp"
    bar.run("mkdir -p " + dir + "; " + script)
  }

  function moveCursor(delta) {
    if (!cursorActive) { cursorActive = true; return }
    var n = actions.length
    cursorIndex = (cursorIndex + delta + n) % n
  }

  function activateCursor() {
    if (cursorIndex < 0 || cursorIndex >= actions.length) return
    capture(actions[cursorIndex].command)
  }

  onOpenedChanged: {
    if (opened) { cursorActive = false; cursorIndex = 0 }
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰄀"
    tooltipText: "Screenshot"
    onPressed: function(b) {
      if (b === Qt.RightButton) root.capture("screenshot region")
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(300))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        if (dy !== 0) root.moveCursor(dy)
        else if (dx !== 0) root.moveCursor(dx)
      }
      onActivateRequested: if (root.cursorActive) root.activateCursor()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(10)

        // ---------- Header ----------
        Item {
          width: parent.width
          implicitHeight: Math.max(heroIcon.implicitHeight, heroLabels.implicitHeight)

          Text {
            id: heroIcon
            textFormat: Text.PlainText
            text: "󰄀"
            color: root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.display
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
          }

          Column {
            id: heroLabels
            anchors.left: heroIcon.right
            anchors.leftMargin: Style.space(14)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            Text {
              text: "Screenshot"
              color: root.bar.foreground
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              elide: Text.ElideRight
              width: parent.width
            }

            Text {
              textFormat: Text.PlainText
              text: ("Saves to " + root.directory.replace(/^~\//, "")).toUpperCase()
              color: Qt.darker(root.bar.foreground, 1.4)
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.2
              elide: Text.ElideRight
              width: parent.width
            }
          }
        }

        PanelSeparator { foreground: root.bar.foreground }

        // ---------- Capture modes ----------
        Column {
          width: parent.width
          spacing: Style.space(2)

          Repeater {
            model: root.actions

            CursorSurface {
              id: row
              required property var modelData
              required property int index

              width: parent.width
              implicitHeight: rowContent.implicitHeight + Style.space(12)
              hasCursor: root.cursorActive && root.cursorIndex === index
              foreground: root.bar.foreground

              MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onContainsMouseChanged: if (containsMouse) {
                  root.cursorActive = true
                  root.cursorIndex = row.index
                }
                onClicked: root.capture(row.modelData.command)
              }

              Item {
                id: rowContent
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: Style.space(10)
                anchors.rightMargin: Style.space(10)
                implicitHeight: Math.max(rowIcon.implicitHeight, rowText.implicitHeight)

                Text {
                  id: rowIcon
                  textFormat: Text.PlainText
                  text: row.modelData.icon
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.title
                  width: Style.space(22)
                  horizontalAlignment: Text.AlignHCenter
                  anchors.left: parent.left
                  anchors.verticalCenter: parent.verticalCenter
                }

                Column {
                  id: rowText
                  anchors.left: rowIcon.right
                  anchors.leftMargin: Style.space(10)
                  anchors.right: parent.right
                  anchors.verticalCenter: parent.verticalCenter
                  spacing: Style.space(1)

                  Text {
                    textFormat: Text.PlainText
                    text: row.modelData.title
                    color: root.bar.foreground
                    font.family: root.bar.fontFamily
                    font.pixelSize: Style.font.body
                    elide: Text.ElideRight
                    width: parent.width
                  }

                  Text {
                    visible: text !== ""
                    textFormat: Text.PlainText
                    text: row.modelData.subtitle
                    color: root.bar.foreground
                    opacity: 0.6
                    font.family: root.bar.fontFamily
                    font.pixelSize: Style.font.caption
                    elide: Text.ElideRight
                    width: parent.width
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
