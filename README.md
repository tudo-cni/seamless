# SEAMLESS

## Disclaimer
This repository is currently under construction. More detailed information will follow shortly. Please expect changes to the README and file structure.

# Publications using SEAMLESS
- T. Gebauer, M. Patchou, C. Wietfeld, "SEAMLESS: Radio Metric Aware Multi-Link Transmission for Resilient Rescue Robotics", In 2023 IEEE International Symposium on Safety, Security, and Rescue Robotics (SSRR), Fukushima, Japan, November 2023.
- M. Patchou, T. Gebauer, C. Krieger, S. Böcker, C. Wietfeld, "Distributed Realtime Wireless Network Emulation for Multi-Robot and Multi-Link Setup Evaluation", In 2023 IEEE International Symposium on Safety, Security, and Rescue Robotics (SSRR), Fukushima, Japan, November 2023.
- M. Patchou, T. Gebauer, F. Schmickmann, S. Böcker, C. Wietfeld, "Immersive Situational Awareness for Robotic Assistance of First Responders Enabled by Reliable 6G Multi-X Communications," in International Conference on 6G Networking (6GNet), Paris, France, October 2024. 

If you use this code or results in your publications, please cite our work as mentioned in [Citation](#citation). Also, if you do not find your work in this list, please open a merge request. 

## Acknowledgements
This work has been funded by the German Federal Ministry of Education and Research (BMBF) via the project LARUS-PRO under funding reference 14N15666 and is further supported by the project DRZ (Establishment of the German Rescue Robotics Center) under funding reference 13N16476, and the 6GEM research hub under funding reference 16KISK038.

## Building 
1. Install [Rust](https://www.rust-lang.org/tools/install).
2. Clone the Repository:
    ```sh
    git clone https://github.com/tudo-cni/seamless
    ```
3. Build the executable (Release flag is important for performance):
    ```
    cargo build --release
    ```
    <details>
    <summary>Enable Tracing</summary>
    
    You may want to enable the tracing feature at the cost of performance.
    To build with tracing:
    ```sh
    cargo build -F tracing --release
    ```
    </details>

4. (Optional) Make the executable systemwide accessible:
    ```
    sudo cp ./target/release/seamless /usr/local/bin/seamless
    ```
## Packaging

Currently SEAMLESS supports .rpm and .deb packages. More to come.

### RPM Package Manager (RPM)
1. Install `generate-rpm` crate:
    ```bash
    cargo install cargo-generate-rpm
    ```
2. Build the project with preferred feature set (see [Building section](#building), release build is highly recommended).
3. Strip the binary:
    ```bash
    strip -s target/release/seamless # In this case native release build, substitute path for different build configurations
    ```
4. Generate .rpm package:
    ```bash
    cargo generate-rpm
    ```
5. You'll find the generated .rpm package under `target/generate-rpm/`

### Debian Package Manager (DEB)
1. Install `cargo-deb` crate:
    ```bash
    cargo install cargo-deb
    ```
2. Generate .deb package:
    ```bash
    cargo deb
    ```
3. You'll find the generated .deb package under `target/debian/seamless/`

## Running SEAMLESS 

1. Configure a `config.yaml` file (filename can be chosen freely) like the [example.yaml](./example.yaml).
2. Currently SEAMLESS requires superuser privileges so the executable must always be run with `sudo`
   ```
    sudo path/to/seamless -c config.yaml
    ```
3. A more detailed tutorial will follow shortly.

## Debugging

### Debugging SEAMLESS protocol flow with wireshark 
1. Install [Wireshark](https://www.wireshark.org).
2. Open Wireshark and go to `Help`->`About Wireshark`->`Folders` and search for either the `Global Lua Plugins` or the `Gobal Plugins` path. E.g. for Fedora 38 it's  `/usr/lib64/wireshark/plugins/4.0`
3. Copy the [dissector.lua](./dissector.lua) to the plugin directory (Example for Fedora 38. You will need to substitute the path):
    ```sh 
    sudo cp ./dissector.lua /usr/lib64/wireshark/plugins/4.0
    ```
4. Fire up Wireshark (might need superuser privileges) go to `Help`->`About Wireshark`->`Plugins` and check if the plugin is loaded.
5. Happy capturing!
6. (Optional) Depending on your selected ports in the `config.yaml` you might need do change the given ports in the [dissector.lua](./dissector.lua) file in the following line:
    ```lua
    udp_table:add(CHANGE_ME_TO_CORRECT_PORT,multilink_proto)
    ```
# Citation
If you use this code or results in your paper, please cite our work as:
```
@InProceedings{gebauer2023,
	Author = {Tim Gebauer and Manuel Patchou and Christian Wietfeld},
	Title = {{SEAMLESS: Radio Metric Aware Multi-Link Transmission for Resilient Rescue Robotics}},
	Booktitle = {2023 IEEE International Symposium on Safety, Security, and Rescue Robotics (SSRR)},
	Address = {Fukushima, Japan},
	Month = {nov},
	Year = {2023},
	Keywords = {Rescue Robotics; Multi-Link; 5G},
	Project = {LARUS-PRO, DRZ, 6GEM},
}
```
