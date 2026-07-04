## cbstream

This is a tool for recording live streams from adult entertainment platforms. It monitors streamers you add and automatically downloads their broadcasts when they go live. It currently supports **CB** (Chaturbate), **SC** (StripChat), **SCVR** (StripChat VR), **BONGA** (BongaCams), and **MFC** (MyFreeCams).


---

### Installation

Install **ffmpeg** if you plan to output in MKV format. No other dependencies are required for basic functionality.

---

### Usage

This is a command line program, to run the program using terminal in Windows:

```powershell
.\cbstream.exe
```

You can optionally pass the path to the configuration file as the first argument. If omitted, the program uses `cb-config.json` in the working directory.

After execution, a JSON configuration file will be saved where set. Add model names to this file to start downloading their streams. The program actively monitors the JSON file, so no restart is needed when adding or removing models.

---

### JSON Configuration

The configuration file follows this structure:

```json
{
  "platform": {
      "CB": ["model1", "model2"],
      "SC": ["model4"],
      "SCVR": [],
      "BONGA": [],
      "MFC": ["model3"]
  },
  "config": {
      "user-agent": ""
  }
}
```

- **CB** (Chaturbate), **SC** (StripChat), **SCVR** (StripChat VR), **BONGA** (BongaCams), **MFC** (MyFreeCams): Supported platforms. Add model names to the respective lists.

Downloaded streams are saved in the working directory inside folders named after each model.

---

### Environment Variables

An optional environment variable `TEMP` can be set to specify where temporary streams are saved (Linux default: `/var/tmp`).

An optional environment variable `CONFIG` can be set to specify a custom path for the configuration file (overrided by the CLI argument).

---

### Docker Usage

To run the program using Docker, use the following command:

```bash
docker run --name cbstream -v <save location>:/cbstream --stop-timeout 300 -itd ghcr.io/baconator696/cbstream:latest && \
    docker logs -f cbstream
```

- Replace `<save location>` with the directory on your host machine where you want to store downloaded files and the config file.
- FFmpeg is bundled into the Docker image.
- The `--stop-timeout 300` flag ensures the container has 300 seconds to shut down gracefully.

---

### To do
- Select maximum resolution
- Add support for more streaming platforms
- Implement the ability to download private shows

---

This project is actively being developed. Contributions and feedback are welcome!
