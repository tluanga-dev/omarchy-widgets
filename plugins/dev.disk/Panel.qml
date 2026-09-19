import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Realtime disk pill for the bar plus a popup with every mounted filesystem
// and the processes doing the most disk I/O. Stats come from the sibling
// `disk-stats` script, which diffs /proc against its previous sample; the
// per-process scan only runs while the popup is open. Each widget instance
// (one per monitor) keeps its own state file so the rates stay correct.
Panel {
  id: root
  moduleName: "dev.disk"
  ipcTarget: "dev.disk"

  readonly property int interval: Math.max(1, Number(setting("interval", 1))) * 1000
  readonly property string mount: String(setting("mount", "/"))
  readonly property bool groupByName: setting("groupByName", true) === true
  readonly property int topCount: Math.max(1, Number(setting("topCount", 5)))

  property var filesystems: []   // [{ target, size, used, avail, source }], bar's mount first
  property real readRate: 0      // bytes/s
  property real writeRate: 0
  property var processes: []     // [{ name, count, rate }]

  readonly property var primary: filesystems.length > 0 ? filesystems[0] : null
  readonly property real fraction: primary && primary.size > 0 ? primary.used / primary.size : 0
  readonly property int pct: Math.round(fraction * 100)
  readonly property real topRate: processes.length > 0 ? Math.max(processes[0].rate, 1) : 1
  readonly property bool critical: pct >= 90
  readonly property string statsScript: Qt.resolvedUrl("disk-stats").toString().replace(/^file:\/\//, "")
  readonly property string stateFile: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp")
    + "/dev.disk-stats." + Math.floor(Math.random() * 1e9) + ".state"

  readonly property real openPanelIndicatorWidth: !button.vertical ? button.labelWidth : 0

  function fmt(bytes) {
    var gb = 1024 * 1024 * 1024
    if (bytes >= 1024 * gb) return (bytes / gb / 1024).toFixed(2) + " TB"
    if (bytes >= gb) return (bytes / gb).toFixed(bytes >= 100 * gb ? 0 : 1) + " GB"
    if (bytes >= 1024 * 1024) return Math.round(bytes / 1024 / 1024) + " MB"
    return Math.round(bytes / 1024) + " kB"
  }

  function fmtRate(bytesPerSec) {
    if (bytesPerSec >= 1024 * 1024) return (bytesPerSec / 1024 / 1024).toFixed(1) + " MB/s"
    if (bytesPerSec >= 1024) return Math.round(bytesPerSec / 1024) + " kB/s"
    return Math.round(bytesPerSec) + " B/s"
  }

  function parse(raw) {
    var lines = String(raw || "").trim().split("\n")
    var fs = []
    var procs = []
    var stats = {}
    for (var i = 0; i < lines.length; i++) {
      var eq = lines[i].indexOf("=")
      if (eq < 0) continue
      var key = lines[i].slice(0, eq)
      var fields = lines[i].slice(eq + 1).split("\t")
      if (key === "fs") {
        fs.push({ target: fields[0], size: Number(fields[1]), used: Number(fields[2]), avail: Number(fields[3]), source: fields[4] })
      } else if (key === "proc") {
        procs.push({ rate: Number(fields[0]), count: Number(fields[1]), name: fields.slice(2).join("\t") })
      } else {
        stats[key] = fields[0]
      }
    }
    // Keep the last good sample if df came back empty.
    if (fs.length === 0) return
    filesystems = fs
    readRate = Number(stats.read) || 0
    writeRate = Number(stats.write) || 0
    if (opened) processes = procs
  }

  function refresh() {
    if (statsProc.running) return
    var cmd = [statsScript, "--state", stateFile, "--mount", mount]
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

  Timer {
    interval: root.interval
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  // The first sample after opening carries no per-process deltas, so clear
  // the stale list and let the next tick fill it.
  onOpenedChanged: {
    if (opened) { processes = []; refresh() }
  }

  Component.onDestruction: Quickshell.execDetached(["rm", "-f", stateFile])

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: button.vertical ? root.pct + "%" : "󰋊 " + root.pct + "%"
    active: root.critical
    tooltipText: root.primary
      ? root.mount + " " + root.fmt(root.primary.used) + " / " + root.fmt(root.primary.size)
      : "Disk"
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
            text: "󰋊"
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
              text: "Disk " + root.mount
              color: root.bar.foreground
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              elide: Text.ElideRight
              width: parent.width
            }

            Text {
              textFormat: Text.PlainText
              text: root.primary
                ? (root.fmt(root.primary.used) + " of " + root.fmt(root.primary.size)).toUpperCase()
                : "—"
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
            InfoPair { label: "Free"; value: root.primary ? root.fmt(root.primary.avail) : "—" }
            InfoPair { label: "Device"; value: root.primary ? root.primary.source.replace(/^\/dev\//, "") : "—" }
          }

          Column {
            width: (parent.width - parent.spacing) / 2
            spacing: Style.spacing.labelGap
            InfoPair { label: "Read"; value: root.fmtRate(root.readRate) }
            InfoPair { label: "Write"; value: root.fmtRate(root.writeRate) }
          }
        }

        // ---------- Other filesystems ----------
        PanelSeparator {
          visible: root.filesystems.length > 1
          foreground: root.bar.foreground
        }

        Column {
          visible: root.filesystems.length > 1
          width: parent.width
          spacing: Style.space(8)

          PanelSectionHeader {
            text: "OTHER FILESYSTEMS"
            foreground: root.bar.foreground
            fontFamily: root.bar.fontFamily
          }

          Repeater {
            model: root.filesystems.slice(1)

            Column {
              required property var modelData
              width: parent.width
              spacing: Style.space(4)

              readonly property real fsFraction: modelData.size > 0 ? modelData.used / modelData.size : 0

              Row {
                width: parent.width
                spacing: Style.space(8)

                Text {
                  textFormat: Text.PlainText
                  text: modelData.target
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  elide: Text.ElideMiddle
                  width: parent.width - fsSize.implicitWidth - parent.spacing
                }

                Text {
                  id: fsSize
                  textFormat: Text.PlainText
                  text: root.fmt(modelData.used) + " / " + root.fmt(modelData.size)
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  font.bold: true
                }
              }

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
                  color: fsFraction >= 0.9 ? root.bar.urgent : root.bar.foreground
                  opacity: 0.7
                  width: Math.max(height, parent.width * fsFraction)

                  Behavior on width { NumberAnimation { duration: 320; easing.type: Easing.OutCubic } }
                }
              }
            }
          }
        }

        // ---------- Top I/O processes ----------
        PanelSeparator { foreground: root.bar.foreground }

        Column {
          width: parent.width
          spacing: Style.space(8)

          PanelSectionHeader {
            text: "TOP " + root.topCount + " DISK I/O"
            foreground: root.bar.foreground
            fontFamily: root.bar.fontFamily
          }

          Text {
            visible: root.processes.length === 0
            text: "Sampling…"
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
                  textFormat: Text.PlainText
                  text: (index + 1) + ". " + modelData.name
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  elide: Text.ElideRight
                  width: parent.width - procCount.implicitWidth - procRate.implicitWidth - parent.spacing * 2
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
                  id: procRate
                  textFormat: Text.PlainText
                  text: root.fmtRate(modelData.rate)
                  color: root.bar.foreground
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  font.bold: true
                }
              }

              // Relative bar: the busiest process fills the width, the rest scale to it.
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
                  width: Math.max(height, parent.width * (modelData.rate / root.topRate))

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
