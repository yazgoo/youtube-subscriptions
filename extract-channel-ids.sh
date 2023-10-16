urls=$(grep -o -E 'https://www.youtube.com/@[^"]*' "$1" | sort | uniq)
for url in $urls
do
  curl -s "$url" | grep -o -E 'https://www.youtube.com/channel/[^"]*' | head -1 | sed 's,.*/,,'
done

