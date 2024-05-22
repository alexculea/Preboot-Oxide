#!/bin/bash

# Initialize version numbers
major=0
minor=0
patch=0

# Fetch all commit messages
commits=$(git log --pretty=format:%s)

# Parse commit messages and update version numbers
while IFS= read -r commit; do
  if [[ $commit =~ ^feat ]]; then
    ((minor++))
  elif [[ $commit =~ ^fix ]]; then
    ((patch++))
  elif [[ $commit =~ ^refactor ]]; then
    ((patch++))
  elif [[ $commit =~ ^performance ]]; then
    ((patch++))
  fi

  if [[ $commit =~ ^BREAKING\ CHANGE ]] || [[ $commit =~ '!:' ]]; then
    ((major++))
    minor=0
    patch=0
  fi
done <<< "$commits"

# Output the version number
printf "$major.$minor.$patch"
