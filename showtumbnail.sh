find ytsthumbnail.jpg | entr -s '
clear
 blockish ytsthumbnail.jpg
toilet -f smmono9 $(head -1 ytsthumbnail.jpg.txt)
tail -1 ytsthumbnail.jpg.txt
echo
 '
