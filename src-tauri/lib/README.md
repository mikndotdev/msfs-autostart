# SimConnect.dll

This is the x64 `SimConnect.dll` from the MSFS 2024 SDK (`<SDK>\SimConnect SDK\lib\SimConnect.dll`). It ships
with the app, so users never need the SDK, and it can't be changed in the app.

- **Release builds** bundle it as a resource (`tauri.conf.json` → `bundle.resources`). The build fails if the file
  is missing, so an installer can't ship without it.
- **Dev builds** load it directly from this folder.

The 2024 version works with both MSFS 2020 and 2024, and it's needed to list streamed MSFS 2024 aircraft. To
update it, replace this file with a newer one from the SDK.
