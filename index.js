const rust = import('./pkg/audio');

rust
  .then(audio => {
        let state = null;
        window.audioStart = function() {
            if (state) {
                return;
            }
            state = audio.start('ws://127.0.0.1:14520', 1, 16000, 320);
            // setTimeout(() => {
            //     audioStop();
            // }, 2000);
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
