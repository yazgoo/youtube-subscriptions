if [ -n "$1" ]; then
  clear
  blockish ytsthumbnail.jpg
  echo -e "\033[10A"
  toilet -f smmono9 $(head -1 ytsthumbnail.jpg.txt) -F border -t
  tail -1 ytsthumbnail.jpg.txt
  exit
fi
find ytsthumbnail.jpg | entr "$0" inner
