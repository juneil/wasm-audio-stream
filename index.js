const rust = import('./pkg/audio');

rust
  .then(audio => {
        let state = null;
        window.audioStart = function() {
            if (state) {
                return;
            }
            state = audio.start(
                new audio.AudioConfig('ws://localhost:14520', 1, 16000, 320)
            );
        }
        window.audioStop = function() {
            if (!state) {
                return;
            }
            audio.stop(state);
            state = null;
        }
  })
  .catch(console.error);
