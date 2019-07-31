# Play an Audio Stream with WASM

## Docker Audio Stream
1. Build the image: 
    - `docker build --tag audiostream:latest .`
2. Run a container with the gstreamer pipeline:
    - `docker run -p 15000:15000 audiostream websockify 15000 -- gst-launch-1.0 audiotestsrc wave=2 freq=200 ! audioconvert ! audioresample ! audio/x-raw,channels=1,rate=16000,format=S16LE ! audiomixer blocksize=320 ! tcpserversink port=15000 host=0.0.0.0`