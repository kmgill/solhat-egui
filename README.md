# SolHat UI

This project aims to provide a desktop user interface for the [SolHat](https://github.com/kmgill/solhat) project using Rust and the [egui](https://github.com/emilk/egui) toolkit. 

As a whole, SolHat is a tool for the stacking of solar and lunar astrophotography, aimed primarily at users of azimuth/elevation mounts (though not exclusively) by providing computationally derived parallactic rotation along with center-of-mass alignment. Frame calibration, analysis, limb darkening correction, and drizzle-enabled stacking are among the core functionality.

The program currently uses `.ser` formatted files as inputs due to their non-compressed image storage and timestamping. Inputs include lights, darks, flats, darkflats, and bias frames; Only the lights are required to produce an output. An optional hot pixel map in json format can be used to replace hot/stuck camera sensor pixels. 

![Main Screen](assets/screenshot-1.jpg)
![Analysis](assets/screenshot-2.jpg)

## Build
### Fedora
Install [Rust](rust-lang.org), then execute the following the ensure the correct dependencies are present:
```bash
sudo dnf group install -y "Development Tools"
sudo dnf install -y gtk3-devel
```

### Ubuntu
Install [Rust](rust-lang.org). Most version of Ubuntu, as of this writing, don't seem to support GTK4 yet, with the exception of `22.10` Kinetic Kudu. 
You will need to execute the following to ensure the correct dependencies are present: 
```bash
sudo apt-get update 
sudo apt-get install -y libgtk-3-dev
```

### Windows
To build in Windows (natively, not in Windows Subsystem for Linux), install the latest versions of MS Visual Studio (Community edition is sufficient), and Rust. 

## Build Installable Packages
Builds targetting `.rpm` and `.deb` packages are done in docker containers. Please ensure Docker is installed (host can be either Linux or Windows).

### RPM (Fedora, Red Hat, etc)
Building for `.rpm` is done by kicking off the `dockerbuild-fedora.sh` script. Using the `docker/Dockerfile.fedora` can also be used to build a container with Solhat-UI installed.

### DEB (Debian, Ubuntu, etc)
Building for `.deb` is done by kicking off the `dockerbuild-debian.sh` script. Using the `docker/Dockerfile.debian` can also be used to build a container with Solhat-UI installed.

### MSI (Windows Installer)
Building a windows package is done using the WiX toolset. Before building, make sure Wix and `cargo-wix` is installed on your system. The build is done within Windows PowerShell. To create the `.msi`, run:

```bash
cargo wix --no-capture
```

## Hot Pixel Map
The hot pixel mapping file is a json formatted text file which provides the sensor width, height, and x/y coordinates to individual pixels. 

Example: 
```json
sensor_width = 1936
sensor_height = 1216
hotpixels = [
	[ 1169 , 48 ],
	[ 170 , 997 ],
	[ 395 , 733 ],
	[ 1193 , 854 ],
]
```