# CPU Temperature

A separate top-bar thermometer, updated every second by default. Click it for
the current package/die temperature, a graph of the last 60 readings, session
minimum/maximum, and a scrollable list of CPU sensors. Right-click opens `btop`.
Use arrows to scroll, R or Enter to refresh, Escape to close, and Tab to switch panels.

```sh
./install.sh dev.cpu-temperature
omarchy bar move dev.cpu-temperature --section right --after dev.cpu
```

Settings in the bar's widget settings UI or `~/.config/omarchy/shell.json`:

```json
{ "id": "dev.cpu-temperature", "interval": 1, "warningTemperature": 80 }
```

`interval` is clamped to 1–60 seconds. `warningTemperature` is a configurable
visual threshold in Celsius (default 80°C), not a claimed hardware safety limit.
The popup separately shows the kernel-reported critical limit when available.
No notifications, thermal limits, or fan settings are changed.

The reader uses Python 3's standard library and read-only sysfs files. It needs
neither `sensors`, root privileges, nor an extra background service.

- Intel `coretemp`: the hottest package across sockets; cores are the fallback
  if package readings are unavailable.
- AMD `k10temp` / `zenpower`: prefers `Tdie`. If only `Tctl` is available, the popup
  explicitly identifies it as a CPU control reading, which may include an offset.
- Also accepts `k8temp`, `via_cputemp`, and CPU-specific thermal drivers/zones.
- Generic ACPI, motherboard, GPU, disk, and Wi-Fi sensors are excluded. A machine
  with no supported CPU sensor displays **—°C** and an explanation.
- Faulty, disabled, malformed, and stale readings are not shown as live values.

The graph uses an automatically scaled vertical range. Each monitor's widget
keeps its own history in memory; the min/max reset when the widget reloads.
Physical sensor update rates depend on the hardware and kernel driver.

Data format references: [hwmon sysfs](https://docs.kernel.org/hwmon/sysfs-interface.html),
[Intel coretemp](https://docs.kernel.org/hwmon/coretemp.html),
[AMD k10temp](https://docs.kernel.org/hwmon/k10temp.html).

## Validation

The helper was checked against fixtures for multiple Intel packages, AMD
Tdie/Tctl selection, excluded GPU/ACPI readings, faulty/disabled/malformed
sensors, and CPU thermal-zone fallback. All five checks passed.
`temperature-stats --sysfs-root <fixture-directory>` permits repeatable fixture
checks without changing real sensors. On the development machine it detects
Intel Package id 0 plus 24 core sensors, with a reported 100°C critical limit.
On Omarchy 4.0.4 / Hyprland 0.56.2, the installed bar label and popup updated
live; the helper matched `sensors` at 68°C in a paired reading. The popup graph
and per-core list rendered, and arrow scrolling retained its position through
subsequent readings. QML parsing and Python compilation passed. Non-Intel
hardware was checked with fixtures, not on physical AMD/ARM machines.

```sh
plugins/dev.cpu-temperature/temperature-stats | python3 -m json.tool
```
