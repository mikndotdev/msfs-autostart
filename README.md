# MSFS Autostart

<p align="center">
 <img src="https://cdn.mikn.dev/branding/mikan-vtube.png" width="100">
</p>

A tray app for MSFS 2020/2024 that starts and closes companion programs automatically, either with the sim or with specific aircraft. Useful for planes that need a companion app, but require you to launch it manually.

## Features

- **Lightweight:** Uses around 150-200MB of RAM and basically no CPU, so is easy to keep running, compared to usually bulky software bundled with addon planes.
- **No Community package:** It talks to the sim through SimConnect, and automatically detects when you spawn into specific aircraft.
- **Intuitive aircraft selection:** Installed aircraft are found by reading each sim's `InstalledPackagesPath`, and the app includes an intuitive aircraft selection menu to make selecting your desired aircraft easier.
- **Highly configurable launch rules:** "with the sim" rules are active while `FlightSimulator.exe`/`FlightSimulator2024.exe` is running. "With aircraft" rules are active while you're in a flight with a matching aircraft. A program closes once its rule stops being active, after a configurable grace period.
