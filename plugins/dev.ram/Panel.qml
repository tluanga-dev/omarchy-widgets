import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui

// Realtime RAM pill for the bar plus a popup listing the biggest memory
// users. Stats come from the sibling `ram-stats` script: the bar polls only
// /proc/meminfo, and the (costlier) process scan runs only while the popup
// is open.
Panel {
  id: root
  moduleName: "dev.ram"
  ipcTarget: "dev.ram"

  readonly property int interval: Math.max(1, Number(setting("interval", 1))) * 1000
  readonly property bool showPercent: setting("showPercent", true) === true
  readonly property bool groupByName: setting("groupByName", true) === true
  readonly property int topCount: Math.max(1, Number(setting("topCount", 5)))

  // All sizes in kB, straight from /proc/meminfo.
  property real totalKb: 0
  property real usedKb: 0
  property real availKb: 0
  property real swapTotalKb: 0
  property real swapUsedKb: 0
  property int pct: 0
  property var processes: []   // [{ name, count, rss }]

  readonly property real fraction: totalKb > 0 ? usedKb / totalKb : 0
  readonly property real topRss: processes.length > 0 ? processes[0].rss : 1
  readonly property bool critical: pct >= 90
  readonly property string statsScript: Qt.resolvedUrl("ram-stats").toString().replace(/^file:\/\//, "")

  readonly property real openPanelIndicatorWidth: !button.vertical ? button.labelWidth : 0

  function fmt(kb) {
    if (kb >= 1024 * 1024) return (kb / 1024 / 1024).toFixed(kb >= 10 * 1024 * 1024 ? 1 : 2) + " GB"
    if (kb >= 1024) return Math.round(kb / 1024) + " MB"
    return Math.round(kb) + " kB"
  }

  function parse(raw) {
    var lines = String(raw || "").trim().split("\n")
    var stats = {}
    var procs = []
    for (var i = 0; i < lines.length; i++) {
      var eq = lines[i].indexOf("=")
      if (eq < 0) continue
      var key = lines[i].slice(0, eq)
      var value = lines[i].slice(eq + 1)
      if (key === "proc") {
        var fields = value.split("\t")
        procs.push({ rss: Number(fields[0]), count: Number(fields[1]), name: fields.slice(2).join("\t") })
      } else {
        stats[key] = Number(value)
      }
    }
    // Keep the last good sample if a poll came back empty.
    if (!(stats.total > 0)) return
    totalKb = stats.total
    usedKb = stats.used
    availKb = stats.avail
    swapTotalKb = stats.swaptotal
    swapUsedKb = stats.swapused
    pct = stats.pct
    if (opened) processes = procs
  }

  function refresh() {
    if (statsProc.running) return
    var cmd = [statsScript]
    if (opened) {
      cmd.push("--top", String(topCount))
      if (groupByName) cmd.push("--group")
    }
    statsProc.command = cmd
    statsProc.running = true
  }

  Process {
    id: statsProc
    stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.parse(text) }
  }

  // One timer feeds both the bar label and the popup; opening the popup
  // just widens what the script reports.
  Timer {
    interval: root.interval
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  onOpenedChanged: if (opened) refresh()

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: button.vertical
      ? (root.showPercent ? root.pct + "%" : "󰍛")
      : (root.showPercent ? "󰍛 " + root.pct + "%" : "󰍛 " + root.fmt(root.usedKb))
    active: root.critical
    tooltipText: "RAM " + root.fmt(root.usedKb) + " / " + root.fmt(root.totalKb)
    onPressed: function(b) {
      if (b === Qt.RightButton) { if (root.bar) root.bar.run("omarchy-launch-or-focus-tui btop") }
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
    contentWidth: panel.fittedContentWidth(Style.space(360))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(14)

        // ---------- Hero: icon · title/status · percentage ----------
        Item {
          width: parent.width
          implicitHeight: Math.max(heroIcon.implicitHeight, heroLabels.implicitHeight, heroPercent.implicitHeight)

          Text {
            id: heroIcon
            textFormat: Text.PlainText
            text: "󰍛"
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
            anchors.right: heroPercent.left
            anchors.rightMargin: Style.space(10)
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            Text {
              text: "Memory"
              color: root.bar.foreground
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              elide: Text.ElideRight
              width: parent.width
            }

            Text {
              textFormat: Text.PlainText
              text: (root.fmt(root.usedKb) + " of " + root.fmt(root.totalKb)).toUpperCase()
              color: Qt.darker(root.bar.foreground, 1.4)
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.2
              elide: Text.ElideRight
              width: parent.width
            }
          }

          Text {
            id: heroPercent
            textFormat: Text.PlainText
            text: root.pct + "%"
            color: root.critical ? root.bar.urgent : root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.displayLarge
            font.bold: true
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter

            Behavior on color { ColorAnimation { duration: 200 } }
          }
        }

        // ---------- Usage bar ----------
        Item {
          width: parent.width
          implicitHeight: Style.space(8)

          Rectangle {
            id: track
            anchors.fill: parent
            radius: height / 2
            color: Qt.rgba(root.bar.foreground.r, root.bar.foreground.g, root.bar.foreground.b, 0.12)
          }

          Rectangle {
            anchors.left: track.left
            anchors.verticalCenter: track.verticalCenter
            height: track.height
            radius: track.radius
            color: root.critical ? root.bar.urgent : root.bar.foreground
            width: Math.max(track.height, track.width * root.fraction)

            Behavior on width { NumberAnimation { duration: 320; easing.type: Easing.OutCubic } }
            Behavior on color { ColorAnimation { duration: 220 } }
          }
        }

        // ---------- Stats ----------
        Row {
          width: parent.width
          spacing: Style.space(20)

          Column {
            width: (parent.width - parent.spacing) / 2
            spacing: Style.spacing.labelGap
            InfoPair { label: "Used"; value: root.fmt(root.usedKb) }
            InfoPair { label: "Available"; value: root.fmt(root.availKb) }
          }

          Column {
            width: (parent.width - parent.spacing) / 2
            spacing: Style.spacing.labelGap
            InfoPair { label: "Total"; value: root.fmt(root.totalKb) }
            InfoPair {
              label: "Swap"
              value: root.swapTotalKb > 0 ? root.fmt(root.swapUsedKb) + " / " + root.fmt(root.swapTotalKb) : "—"
            }
          }
        }

        // ---------- Top processes ----------
        PanelSeparator { foreground: root.bar.foreground }

        Column {
          width: parent.width
          spacing: Style.space(8)

          PanelSectionHeader {
            text: "TOP " + root.topCount + " MEMORY USERS"
            foreground: root.bar.foreground
            fontFamily: root.bar.fontFamily
          }

          Text {
            visible: root.processes.length === 0
            text: "Scanning…"
            color: root.bar.foreground
            opacity: 0.6
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.bodySmall
          }

          Repeater {
            model: root.processes

            Column {
              required property var modelData
              required property int index
              width: parent.width
              spacing: Style.space(4)

              Row {
                width: parent.width
                spacing: Style.space(8)

                Text {
                  id: procName
                  textFormat: Text.PlainText
                  text: (index + 1) + ". " + modelData.name
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  elide: Text.ElideRight
                  width: parent.width - procCount.implicitWidth - procSize.implicitWidth - parent.spacing * 2
                }

                Text {
                  id: procCount
                  textFormat: Text.PlainText
                  text: modelData.count > 1 ? "×" + modelData.count : ""
                  color: root.bar.foreground
                  opacity: 0.5
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.caption
                  anchors.verticalCenter: parent.verticalCenter
                }

                Text {
                  id: procSize
                  textFormat: Text.PlainText
                  text: root.fmt(modelData.rss)
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  font.bold: true
                }
              }

              // Relative bar: the top process fills the width, the rest scale to it.
              Item {
                width: parent.width
                implicitHeight: Style.space(3)

                Rectangle {
                  anchors.fill: parent
                  radius: height / 2
                  color: Qt.rgba(root.bar.foreground.r, root.bar.foreground.g, root.bar.foreground.b, 0.08)
                }

                Rectangle {
                  height: parent.height
                  radius: height / 2
                  color: root.bar.foreground
                  opacity: 0.7
                  width: Math.max(height, parent.width * (modelData.rss / root.topRss))

                  Behavior on width { NumberAnimation { duration: 320; easing.type: Easing.OutCubic } }
                }
              }
            }
          }
        }
      }
    }
  }

  component InfoPair: Row {
    property string label: ""
    property string value: ""

    width: parent.width
    spacing: Style.space(8)

    InfoLabel { text: label }
    Item { width: Math.max(0, parent.width - parent.children[0].implicitWidth - parent.children[2].implicitWidth - parent.spacing * 2); height: 1 }
    InfoValue { text: value }
  }

  component InfoLabel: Text {
    textFormat: Text.PlainText
    color: root.bar.foreground
    opacity: 0.6
    font.family: root.bar.fontFamily
    font.pixelSize: Style.font.bodySmall
  }

  component InfoValue: Text {
    textFormat: Text.PlainText
    color: root.bar.foreground
    font.family: root.bar.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
