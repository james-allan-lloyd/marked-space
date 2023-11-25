source .env
curl --request GET \
  --url 'https://jimjim256.atlassian.net/wiki/api/v2/spaces' \
  --user "$API_USER:$API_TOKEN" \
  --header 'Accept: application/json'