#!/bin/sh

set -e

readonly token="$( cat ".gh-token" )"

curl \
  --header "Accept: application/vnd.github.v4.idl" \
  --header "Authorization: bearer $token" \
  "https://api.github.com/graphql" | \
  jq .data --raw-output \
  > schema.graphql

# Double newline because the main schema is without a newline at the end of the
# file.
cat >>schema.graphql <<EOF

schema {
  query: Query
  mutation: Mutation
}
EOF
