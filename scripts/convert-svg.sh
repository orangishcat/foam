# set lucide icons svg from stroke=currentColor (defaults to black in slint) to stroke=white
sed -i '' 's/stroke="currentColor"/stroke="#ffffff"/g' ui/icons/*.svg
