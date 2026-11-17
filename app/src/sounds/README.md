# The widget's recordings

Short recordings the widget can play when a session asks, finishes or stops with an error
(`../sounds.js` lists them; the settings pick which). Each was cut to the sound itself, mixed down to
mono at 22 kHz and brought to one loudness with `docs/sound.mjs`; the originals are at the links.

| File | Recording | By | Licence |
|---|---|---|---|
| `meow.wav` | [Cat Meow](https://freesound.org/people/Mafon2/sounds/436541/) | Mafon2 | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `meow2.wav` | [Cat meow](https://freesound.org/people/TRNGLE/sounds/362652/) | TRNGLE | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `meow3.wav` | [cat meow short](https://freesound.org/people/skymary/sounds/412017/) | skymary | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `quack.wav` | [single quack from a duck](https://freesound.org/people/Mikes-MultiMedia/sounds/418509/) | Mikes-MultiMedia | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `frog.wav` | [Frog.ogg](https://freesound.org/people/egomassive/sounds/536759/) | egomassive | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `cluck.wav` | [Chicken Single Alarm Call](https://freesound.org/people/Rudmer_Rotteveel/sounds/316920/) | Rudmer_Rotteveel | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `plop.wav` | [Plop!](https://freesound.org/people/Breviceps/sounds/447910/) | Breviceps | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `quack2.wav` | the notification sound of Calibri, another project of this widget's author | — | — |

The recordings from [Freesound](https://freesound.org) were released by their authors under CC0:
no conditions, no attribution required. They are named here all the same.

To add one: `node docs/sound.mjs <recording> <name>`, then the name in `FILES` (`../sounds.js`), its
words in `../i18n.js` (`snd.<name>`) and a row here.
