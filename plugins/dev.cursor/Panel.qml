import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "dev.cursor"
  ipcTarget: "dev.cursor"

  readonly property bool autoStart: setting("autoStart", true) === true
  readonly property string helper: decodeURIComponent(Qt.resolvedUrl("cursor-control").toString().replace(/^file:\/\//, ""))
  readonly property string application: Quickshell.env("HOME") + "/.local/bin/omarchy-cursor"
  property var cursorState: ({ running: false, enabled: false, magnifying: false })
  property string message: ""
  property bool cursorActive: false
  property int cursorIndex: 0
  readonly property bool active: cursorState.running === true && cursorState.enabled === true
  readonly property string summary: !cursorState.running ? "Stopped" : cursorState.magnifying ? "Magnifier on" : active ? "Highlight on" : "Highlight paused"
  readonly property real openPanelIndicatorWidth: 0
  readonly property var actions: [
    { icon: "󰇀", title: cursorState.running ? "Cursor highlight" : "Start cursor highlight", subtitle: cursorState.toggle_key || "Super + Alt + C", command: "toggle", checked: active, toggle: true },
    { icon: "󰍉", title: "Magnifier", subtitle: cursorState.magnify_key || "Super + Alt + M", command: "magnify", checked: cursorState.magnifying === true, toggle: true },
    { icon: "󰐾", title: "Locate pointer", subtitle: cursorState.locate_key || "Super + Alt + L", command: "locate" },
    { icon: "󰒓", title: "Preferences", subtitle: "Appearance · magnifier · behavior", command: "settings" },
    { icon: "󰐥", title: cursorState.running ? "Stop cursor helper" : "Start cursor helper", subtitle: "", command: cursorState.running ? "quit" : "start" }
  ]

  function parse(raw) {
    try {
      var next = JSON.parse(String(raw).trim())
      cursorState = next
      message = next.capture_error || next.error || ""
    } catch (error) {
      if (String(raw).trim() !== "") message = String(raw).trim()
    }
  }
  function refresh() {
    if (!statusProcess.running && !actionProcess.running) statusProcess.running = true
  }
  function execute(command) {
    if (command === "settings") {
      close()
      Quickshell.execDetached([application, "settings"])
      return
    }
    if (actionProcess.running) return
    message = ""
    actionProcess.command = [helper, command]
    actionProcess.running = true
  }
  function moveCursor(delta) {
    if (!cursorActive) { cursorActive = true; return }
    cursorIndex = (cursorIndex + delta + actions.length) % actions.length
  }
  onOpenedChanged: if (opened) { cursorActive = false; cursorIndex = 0; refresh() }
  Component.onCompleted: {
    if (autoStart) execute("start")
    else refresh()
  }

  Process {
    id: statusProcess
    command: [root.helper, "status"]
    stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.parse(text) }
  }
  Process {
    id: actionProcess
    stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.parse(text) }
    stderr: StdioCollector { waitForEnd: true; onStreamFinished: if (text.trim()) root.message = text.trim() }
    onExited: refreshTimer.restart()
  }
  Timer { id: refreshTimer; interval: 150; onTriggered: root.refresh() }
  Timer { interval: root.opened ? 500 : 2000; running: true; repeat: true; onTriggered: root.refresh() }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight
  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰇀"
    active: root.active || root.cursorState.magnifying === true
    activeColor: Color.accent
    dimmed: !root.cursorState.running
    tooltipText: "Cursor · " + root.summary + "\nRight-click: toggle highlight · Middle-click: magnifier"
    onPressed: function(b) {
      if (b === Qt.RightButton) root.execute("toggle")
      else if (b === Qt.MiddleButton) root.execute("magnify")
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
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) { root.moveCursor(dy !== 0 ? dy : dx) }
      onActivateRequested: if (root.cursorActive) root.execute(root.actions[root.cursorIndex].command)
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        Item {
          width: parent.width
          implicitHeight: Style.space(56)
          Rectangle {
            id: hero
            width: Style.space(48)
            height: width
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            radius: width / 2
            color: "transparent"
            border.width: Style.space(2)
            border.color: root.active ? Color.accent : Qt.darker(root.bar.foreground, 1.5)
            Text {
              anchors.centerIn: parent
              text: "󰇀"
              textFormat: Text.PlainText
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              color: root.bar.foreground
            }
          }
          Column {
            anchors.left: hero.right
            anchors.leftMargin: Style.space(14)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(3)
            Text {
              text: "Cursor"
              color: root.bar.foreground
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
            }
            Text {
              text: root.summary
              color: root.active ? Color.accent : root.bar.foreground
              opacity: 0.8
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }
        Text {
          width: parent.width
          visible: root.cursorState.running === true
          text: (root.cursorState.shape || "Circle") + " · " + (root.cursorState.radius || 24) + " px · " + (root.cursorState.zoom || 2.5) + "× zoom"
          color: root.bar.foreground
          opacity: 0.6
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.caption
        }
        PanelSeparator { foreground: root.bar.foreground }
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
              implicitHeight: rowLabels.implicitHeight + Style.space(16)
              foreground: root.bar.foreground
              hasCursor: root.cursorActive && root.cursorIndex === index
              opacity: actionProcess.running ? 0.65 : 1
              MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onContainsMouseChanged: if (containsMouse) { root.cursorActive = true; root.cursorIndex = row.index }
                onClicked: root.execute(row.modelData.command)
              }
              Text {
                id: rowIcon
                anchors.left: parent.left
                anchors.leftMargin: Style.space(10)
                anchors.verticalCenter: parent.verticalCenter
                width: Style.space(24)
                text: row.modelData.icon
                textFormat: Text.PlainText
                color: root.bar.foreground
                font.family: root.bar.fontFamily
                font.pixelSize: Style.font.title
              }
              Column {
                id: rowLabels
                anchors.left: rowIcon.right
                anchors.leftMargin: Style.space(10)
                anchors.right: rowSwitch.left
                anchors.rightMargin: Style.space(8)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Style.space(2)
                Text {
                  width: parent.width
                  text: row.modelData.title
                  textFormat: Text.PlainText
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.body
                  elide: Text.ElideRight
                }
                Text {
                  width: parent.width
                  visible: text !== ""
                  text: row.modelData.subtitle
                  textFormat: Text.PlainText
                  color: root.bar.foreground
                  opacity: 0.6
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.caption
                  elide: Text.ElideRight
                }
              }
              ToggleSwitch {
                id: rowSwitch
                anchors.right: parent.right
                anchors.rightMargin: Style.space(10)
                anchors.verticalCenter: parent.verticalCenter
                visible: row.modelData.toggle === true
                width: visible ? implicitWidth : 0
                trackHeight: Style.space(18)
                interactive: false
                checked: row.modelData.checked === true
                foreground: root.bar.foreground
              }
            }
          }
        }
        Text {
          width: parent.width
          visible: root.message !== ""
          text: root.message
          textFormat: Text.PlainText
          color: root.bar.urgent
          wrapMode: Text.Wrap
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
    }
  }
}
