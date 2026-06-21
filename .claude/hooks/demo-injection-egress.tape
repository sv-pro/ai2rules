# vhs tape for the E13.5 prompt-injection -> egress demo (PLAN.md E13.5).
#
# Renders .claude/hooks/demo-injection-egress.sh to an animated GIF for the
# "Running Claude Code Safely" blog post. Reproducible: install vhs
# (https://github.com/charmbracelet/vhs) and run, from the repo root:
#
#   vhs .claude/hooks/demo-injection-egress.tape
#
# Output lands at blog/src/assets/demo-injection-egress.gif so the article can
# reference it directly. The demo itself is self-contained (throwaway temp dir),
# so the recording has no side effects on your session.

Output blog/src/assets/demo-injection-egress.gif

Set Shell bash
Set FontSize 14
Set Width 1280
Set Height 1000
Set Padding 20
Set TypingSpeed 35ms
Set Theme "Catppuccin Mocha"

# Start from a clean prompt.
Hide
Type "clear"
Enter
Show

Type "bash .claude/hooks/demo-injection-egress.sh"
Sleep 600ms
Enter

# The script just prints — give it time to render, then hold on the result so
# viewers can read the verdict before the loop restarts.
Sleep 7s
