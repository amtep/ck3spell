# ck3spell
Spelling checker for Crusader Kings 3 localization files

## Purpose
Localization files for CK3 (Crusader Kings 3) contain the text presented to the user for all the UI elements, in-game events and decisions, etc.
This text often needs to be adjusted depending on the game situation, such as who is involved in an event and which of the involved characters is the player.
Because of that need, the localization texts often contain small blocks of code, such as `[Character.GetCulture.GetName]`, or formatting directives such as `$VALUE|=+0$`.

`ck3spell` is a spelling checker that is optimized for these localization files. It skips the bits of code and other non-text content, and applies its spelling check only to the words that will be shown to the user.

## Screenshot
![application screenshot](./screenshots/sample.png "Sample of the UI")

## Features
* Colors the localization strings to make the user-visible parts easy to recognize
* Uses the correct language from the supported languages for CK3
* Can add words to a permanent local dictionary

## Dependencies
`ck3spell` uses `hunspell` dictionaries but not the `hunspell` library.

On Debian and Ubuntu, you should install one or hunspell dictionary packages depending on which languages you want to check.

* English: `hunspell-en-us`
* German: `hunspell-de-de`
* Spanish: `hunspell-es`
* French: `hunspell-fr`
* Russian: `hunspell-ru`

`sudo apt install hunspell-en-us hunspell-de-de hunspell-es hunspell-fr hunspell-ru`

### Development Dependencies
`ck3spell` uses the `GTK-3` library on Linux.

On Debian and Ubuntu, you should install the `libgtk-3-dev` package before compiling `ck3spell`.

`sudo apt install libgtk-3-dev`

## TODO
* Support Korean and Chinese.
* Check words that are part text and part code. Such combination words are important for some languages which use bits of code to get the word endings right. (for example 'luchador'/'luchadora' in Spanish).
* Support installation and dictionary bundling on Windows and Mac.
* Provide installable executables.
