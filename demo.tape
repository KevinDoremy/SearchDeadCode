# VHS Demo Recording Script
# https://github.com/charmbracelet/vhs
#
# To record the demo GIF, install VHS and run:
#   vhs demo.tape
#
# This will generate assets/demo.gif

Output assets/demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 50ms

# Start with a clean terminal
Hide
Type "clear"
Enter
Sleep 500ms
Show

# Show the command we're about to run
Type "# Analyze an Android project for dead code"
Enter
Sleep 1s

# Run the analysis
Type "searchdeadcode ./my-android-app --min-confidence high"
Enter
Sleep 3s

# Simulate output (this would be real output in practice)
# The actual output will appear when running against a real project

Sleep 2s

# Show safe delete preview
Type ""
Enter
Type "# Preview what would be deleted"
Enter
Sleep 500ms

Type "searchdeadcode ./my-android-app --delete --dry-run"
Enter
Sleep 3s

# Final pause
Sleep 2s
