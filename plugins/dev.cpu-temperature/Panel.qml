import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "dev.cpu-temperature"
  ipcTarget: "dev.cpu-temperature"

  function bounded(value, fallback, min, max) {
    var n = Number(value)
    return isFinite(n) ? Math.max(min, Math.min(max, n)) : fallback
  }
  readonly property int interval: Math.round(bounded(setting("interval", 1), 1, 1, 60) * 1000)
  readonly property real warningTemperature: bounded(setting("warningTemperature", 80), 80, 30, 120)
  readonly property string statsScript: decodeURIComponent(Qt.resolvedUrl("temperature-stats").toString().replace(/^file:\/\//, ""))
  readonly property real openPanelIndicatorWidth: button.vertical ? 0 : button.labelWidth
  property var reading: ({ available: false, sensors: [] })
  property var history: []
  property var minimum: null
  property var maximum: null
  property string message: "Reading CPU sensors…"
  property double lastSample: 0
  property bool received: false
  readonly property bool available: reading.available === true
  readonly property bool critical: available && reading.critical !== null && reading.temperature >= reading.critical
  readonly property bool warm: available && (critical || reading.temperature >= warningTemperature)
  readonly property color temperatureColor: warm ? root.bar.urgent : Color.accent

  function degrees(value, decimal) {
    return typeof value === "number" && isFinite(value) ? (decimal ? value.toFixed(1) : Math.round(value)) + "°C" : "—"
  }
  function unavailable(reason) {
    reading = { available: false, sensors: [] }
    message = reason
    history = []
  }
  function parse(raw) {
    received = true
    try {
      var next = JSON.parse(String(raw))
      if (!next.available || typeof next.temperature !== "number" || !isFinite(next.temperature)) {
        unavailable(next.error || "CPU temperature unavailable")
        return
      }
      if (reading.available && (reading.driver !== next.driver || reading.source !== next.source)) {
        history = []
        minimum = null
        maximum = null
      }
      reading = next
      message = ""
      lastSample = Date.now()
      minimum = minimum === null ? next.temperature : Math.min(minimum, next.temperature)
      maximum = maximum === null ? next.temperature : Math.max(maximum, next.temperature)
      history = history.concat([next.temperature]).slice(-60)
    } catch (error) {
      unavailable("Could not read CPU temperature")
    }
  }
  function refresh() {
    if (statsProcess.running) return
    received = false
    statsProcess.running = true
  }
  function scroll(direction) {
    sensorList.contentY = Math.max(0, Math.min(sensorList.contentHeight - sensorList.height,
                                            sensorList.contentY + direction * Style.space(48)))
  }
  onOpenedChanged: if (opened) refresh()
  onHistoryChanged: chart.requestPaint()
  onTemperatureColorChanged: chart.requestPaint()

  Process {
    id: statsProcess
    command: [root.statsScript]
    stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.parse(text) }
    onExited: function(code) {
      if (code !== 0 || !root.received) root.unavailable("CPU sensor reader unavailable")
    }
  }
  Timer {
    interval: root.interval
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }
  Timer {
    interval: 1000
    running: true
    repeat: true
    onTriggered: {
      if (root.available && Date.now() - root.lastSample > Math.max(5000, root.interval * 3))
        root.unavailable("CPU temperature sample is stale")
    }
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight
  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: (vertical ? "" : "󰔏 ") + (root.available ? root.degrees(root.reading.temperature, false) : "—°C")
    active: root.warm
    dimmed: !root.available
    tooltipText: root.available
      ? "CPU · " + root.degrees(root.reading.temperature, true) + " · " + root.reading.source + "\nClick for sensor details · Right-click: btop"
      : "CPU temperature · " + root.message
    onPressed: function(b) {
      if (b === Qt.RightButton) {
        if (root.bar) root.bar.run("omarchy-launch-or-focus-tui btop")
      } else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(340))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onMoveRequested: function(dx, dy) { root.scroll(dy !== 0 ? dy : dx) }
      onActivateRequested: root.refresh()
      onTextKey: function(text) { if (text.toLowerCase() === "r") root.refresh() }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        Item {
          width: parent.width
          implicitHeight: Style.space(56)
          Column {
            anchors.left: parent.left
            anchors.right: hero.left
            anchors.rightMargin: Style.space(12)
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(4)
            Text {
              width: parent.width
              text: "CPU temperature"
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              color: root.bar.foreground
              elide: Text.ElideRight
            }
            Text {
              width: parent.width
              text: root.available ? root.reading.source : "Sensor unavailable"
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.caption
              color: root.bar.foreground
              opacity: 0.6
              elide: Text.ElideRight
            }
          }
          Text {
            id: hero
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: root.available ? root.degrees(root.reading.temperature, false) : "—"
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.displayLarge
            font.bold: true
            color: root.temperatureColor
          }
        }

        Canvas {
          id: chart
          width: parent.width
          height: visible ? Style.space(68) : 0
          visible: root.available
          onWidthChanged: requestPaint()
          onVisibleChanged: requestPaint()
          onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            if (root.history.length < 2) return
            var low = Math.min.apply(null, root.history) - 3
            var high = Math.max.apply(null, root.history) + 3
            ctx.strokeStyle = Qt.rgba(root.bar.foreground.r, root.bar.foreground.g, root.bar.foreground.b, 0.12)
            ctx.lineWidth = 1
            for (var j = 0; j < 3; j++) {
              var y = 4 + j * (height - 8) / 2
              ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(width, y); ctx.stroke()
            }
            ctx.strokeStyle = root.temperatureColor
            ctx.lineWidth = 2
            ctx.beginPath()
            for (var i = 0; i < root.history.length; i++) {
              var x = i / (root.history.length - 1) * width
              var pointY = height - 4 - (root.history[i] - low) / (high - low) * (height - 8)
              if (i === 0) ctx.moveTo(x, pointY)
              else ctx.lineTo(x, pointY)
            }
            ctx.stroke()
          }
        }

        Text {
          width: parent.width
          visible: root.available
          text: "Session min " + root.degrees(root.minimum, false) + "  ·  max " + root.degrees(root.maximum, false)
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.caption
          color: root.bar.foreground
          opacity: 0.65
        }
        Text {
          width: parent.width
          visible: root.available
          text: root.critical ? "At or above the sensor's critical limit"
            : root.warm ? "Above the configured warning level"
            : "Highlight at " + root.degrees(root.warningTemperature, false)
              + (root.reading.critical !== null ? " · Limit " + root.degrees(root.reading.critical, false) : "")
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.caption
          color: root.warm ? root.bar.urgent : root.bar.foreground
          wrapMode: Text.Wrap
        }

        PanelSeparator { foreground: root.bar.foreground }
        PanelSectionHeader {
          text: "CPU SENSORS"
          foreground: root.bar.foreground
          fontFamily: root.bar.fontFamily
        }
        Flickable {
          id: sensorList
          width: parent.width
          height: Math.min(sensorColumn.implicitHeight, Style.space(200))
          contentWidth: width
          contentHeight: sensorColumn.implicitHeight
          flickableDirection: Flickable.VerticalFlick
          boundsBehavior: Flickable.StopAtBounds
          clip: true
          Column {
            id: sensorColumn
            width: sensorList.width
            spacing: Style.space(6)
            Repeater {
              model: root.reading.sensors || []
              Item {
                required property var modelData
                width: sensorColumn.width
                implicitHeight: sensorLabel.implicitHeight
                Text {
                  id: sensorLabel
                  anchors.left: parent.left
                  anchors.right: sensorValue.left
                  anchors.rightMargin: Style.space(12)
                  text: parent.modelData.label
                  textFormat: Text.PlainText
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.body
                  color: root.bar.foreground
                  elide: Text.ElideRight
                }
                Text {
                  id: sensorValue
                  anchors.right: parent.right
                  text: root.degrees(parent.modelData.celsius, true)
                  font.family: root.bar.fontFamily
                  font.pixelSize: Style.font.body
                  color: parent.modelData.celsius >= root.warningTemperature ? root.bar.urgent : root.bar.foreground
                }
              }
            }
          }
        }
        Text {
          width: parent.width
          visible: root.message !== ""
          text: root.message
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.body
          color: root.bar.foreground
          wrapMode: Text.Wrap
        }
        Text {
          width: parent.width
          text: "Updates every " + (root.interval / 1000) + "s · R to refresh"
            + (sensorList.contentHeight > sensorList.height ? "\nScroll or use arrows for more sensors" : "")
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.caption
          color: root.bar.foreground
          opacity: 0.5
        }
      }
    }
  }
}
