[ "$PWD" = "$FOSSIL_PROJECT_DIR/3-3-experiment" ]
[ "$FOSSIL_NAME" = '3-3-experiment' ]
[ "$FOSSIL_ARTIFACT_NAME" = 'summary' ]
[ "$FOSSIL_CONST_VALUE" = 'present' ]
[ "$FOSSIL_FORCE" = '1' ]
[ -z "$(cat)" ]
printf '{"static":true}' > "$1"
