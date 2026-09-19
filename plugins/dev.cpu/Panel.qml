import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Realtime CPU pill for the bar plus a popup listing the busiest processes.
// Stats come from the sibling `cpu-stats` script, which diffs /proc against
// its previous sample; the per-process scan only runs while the popup is
// open. Each widget instance (one per monitor) keeps its own state file so
// the deltas stay correct.
Panel {
  id: root
  moduleName: "dev.cpu"
  ipcTarget: "dev.cpu"

  readonly property int interval: Math.max(1, Number(setting("interval", 1))) * 1000
  readonly property bool groupByName: setting("groupByName", true) === true
  readonly property int topCount: Math.max(1, Number(setting("topCount", 5)))

  property int pct: 0
  property int ncpu: 0
  property string load: ""
  property string model: ""
  property int mhz: 0
  property string temp: ""
  property var processes: []   // [{ name, count, pct }]

  readonly property real topPct: processes.length > 0 ? Math.max(processes[0].pct, 0.01) : 1
  readonly property bool critical: pct >= 90
  readonly property string statsScript: Qt.resolvedUrl("cpu-stats").toString().replace(/^file:\/\//, "")
  readonly property string stateFile: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp")
    + "/dev.cpu-stats." + Math.floor(Math.random() * 1e9) + ".state"

  readonly property real openPanelIndicatorWidth: !button.vertical ? button.labelWidth : 0

  function fmtPct(value) {
    return value >= 10 ? Math.round(value) + "%" : value.toFixed(1) + "%"
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
        procs.push({ pct: Number(fields[0]), count: Number(fields[1]), name: fields.slice(2).join("\t") })
      } else {
        stats[key] = value
      }
    }
    if (stats.pct === undefined) return
    pct = Number(stats.pct)
    ncpu = Number(stats.ncpu)
    load = stats.load || ""
    model = stats.model || ""
    mhz = Number(stats.mhz) || 0
    temp = stats.temp || ""
    if (opened) processes = procs
  }

  function refresh() {
    if (statsProc.running) return
    var cmd = [statsScript, "--state", stateFile]
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

  // The first sample after opening carries no process deltas, so clear the
  // stale list and let the next tick fill it.
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
    text: button.vertical ? root.pct + "%" : "󰻠 " + root.pct + "%"
    active: root.critical
    tooltipText: "CPU " + root.pct + "% · load " + root.load
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

        // ---------- Hero: icon · title/model · percentage ----------
        Item {
          width: parent.width
          implicitHeight: Math.max(heroIcon.implicitHeight, heroLabels.implicitHeight, heroPercent.implicitHeight)

          Text {
            id: heroIcon
            textFormat: Text.PlainText
            text: "󰻠"
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
              text: "Processor"
              color: root.bar.foreground
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              elide: Text.ElideRight
              width: parent.width
            }

            Text {
              textFormat: Text.PlainText
              text: (root.model || "CPU").replace(/\(R\)|\(TM\)/g, "").replace(/\s+/g, " ").toUpperCase()
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
            width: Math.max(track.height, track.width * root.pct / 100)

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
            InfoPair { label: "Load"; value: root.load || "—" }
            InfoPair { label: "Threads"; value: root.ncpu > 0 ? String(root.ncpu) : "—" }
          }

          Column {
            width: (parent.width - parent.spacing) / 2
            spacing: Style.spacing.labelGap
            InfoPair { label: "Clock"; value: root.mhz > 0 ? (root.mhz / 1000).toFixed(2) + " GHz" : "—" }
            InfoPair { label: "Temp"; value: root.temp !== "" ? root.temp + "°C" : "—" }
          }
        }

        // ---------- Top processes ----------
        PanelSeparator { foreground: root.bar.foreground }

        Column {
          width: parent.width
          spacing: Style.space(8)

          PanelSectionHeader {
            text: "TOP " + root.topCount + " CPU USERS"
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
                  width: parent.width - procCount.implicitWidth - procPct.implicitWidth - parent.spacing * 2
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
                  id: procPct
                  textFormat: Text.PlainText
                  text: root.fmtPct(modelData.pct)
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
                  width: Math.max(height, parent.width * (modelData.pct / root.topPct))

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
