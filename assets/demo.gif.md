# Demo GIF Placeholder

This file is a placeholder for the demo GIF. To generate the actual demo:

## Option 1: Using VHS (Recommended)

[VHS](https://github.com/charmbracelet/vhs) is a tool for recording terminal GIFs.

```bash
# Install VHS
brew install vhs

# Generate the demo GIF
vhs demo.tape
```

This will create `assets/demo.gif` from the `demo.tape` script.

## Option 2: Using asciinema + asciicast2gif

```bash
# Record
asciinema rec demo.cast

# Convert to GIF
docker run --rm -v $PWD:/data asciinema/asciicast2gif demo.cast demo.gif
mv demo.gif assets/
```

## Option 3: Using terminalizer

```bash
npm install -g terminalizer
terminalizer record demo
terminalizer render demo -o assets/demo.gif
```

## After Creating

1. Delete this placeholder file (`assets/demo.gif.md`)
2. Ensure `assets/demo.gif` is under 10MB for GitHub
3. The README will automatically display the GIF

## Quick Recording Script

```bash
# Just run this in the repo root:
searchdeadcode ./tests/fixtures/android --min-confidence high
```

The output should show the colorful terminal display with confidence indicators.
