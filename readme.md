
## cbstream - Work in Progress

This is a work in progress tool for downloading streams from supported platforms. It currently supports **CB**, **SC**, **BONGA**, and **MFC**. For MKV output, ensure **ffmpeg** or **mkvtoolnix** is installed.

---

### Installation

Before using the program, install **ffmpeg** or **mkvtoolnix** if you plan to output in MKV format. No other dependencies are required for basic functionality.

---

### Usage

This is a command line program, to run the program using terminal in Windows:

```powershell
.\cbstream.exe
```

After execution, a JSON configuration file will be saved in the working directory. Add model names to this file to start downloading their streams. The program actively monitors the JSON file, so no restart is needed when adding or removing models.

---

### JSON Configuration

The configuration file follows this structure:

```json
{
  "platform": {
      "CB": ["model1","model2"],
      "MFC": ["model3"],
      "SCVR": [],
      "SC": [],
      "BONGA": [],
  },
  "config": {
      "user-agent": ""
  }
}
```

- **CB**, **SC**, **MFC**, **SCVR**, **BONGA**: Supported platforms. Add model names to the respective lists.

---

### Environment Variables

An optional environment variable `TEMP` can be set to specify where temporary streams are saved. If not set, temporary files will be stored in the OS's default temp folder.

---

### Docker Usage

To run the program using Docker, use the following command:

```bash
docker run --name cbstream -v <save location>:/cbstream --stop-timeout 300 -itd ghcr.io/baconator696/cbstream:latest && \
    docker logs -f cbstream
```

- Replace `<save location>` with the directory on your host machine where you want to store downloaded files.
- The `--stop-timeout 300` flag ensures the container has 300 seconds to shut down gracefully.
- The `-itd` flags run the container in interactive, TTY, and detached mode.
- The JSON configuration file will be saved in the mounted directory (`/cbstream`).

---

### To do
- select maximum resolution
- Add support for more streaming platforms
- Implement the ability to download private shows
- Build for MacOS, but I don't have a mac to test on

---
I will not create binaries for Linux because the Linux binary relies on shared libraries, you'll have to compile it yourself

Rust and Git need to be installed:
```bash
git clone https://github.com/baconator696/cbstream.git
cargo build -r
```
---

This project is actively being developed. Contributions and feedback are welcome!