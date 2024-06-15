#!/bin/env bash

# Returns 0 if version1 is greater than version2
# 1 if version1 is less than version2
# 2 if version1 is equal to version2
compare_versions() {
  local version1=$1
  local version2=$2

  [[ $version1 =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(.+)?$ ]];
  v1_major=${BASH_REMATCH[1]};
  v1_minor=${BASH_REMATCH[2]};
  v1_patch=${BASH_REMATCH[3]};

  [[ $version2 =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(.+)?$ ]];
  v2_major=${BASH_REMATCH[1]};
  v2_minor=${BASH_REMATCH[2]};
  v2_patch=${BASH_REMATCH[3]};

  # Compare major versions
  if [[ $v1_major -gt $v2_major ]]; then
    # echo "Version $version1 is greater than $version2"
    return 0
  elif [[ $v1_major -lt $v2_major ]]; then
    # echo "Version $version1 is less than $version2"
    return 1
  fi

  # Compare minor versions
  if [[ $v1_minor -gt v2_minor ]]; then
    #echo "Version $version1 is greater than $version2"
    return 0
  elif [[ $v1_minor -lt v2_minor ]]; then
    #echo "Version $version1 is less than $version2"
    return 1
  fi

  # Compare patch versions
  if [[ $v1_patch -gt v2_patch ]]; then
    #echo "Version $version1 is greater than $version2"
    echo 0
  elif [[ $v1_patch -lt v2_patch ]]; then
    #echo "Version $version1 is less than $version2"
    return 1
  fi
  # Versions are equal
  # echo "Version $version1 is equal to $version2"
  return 2
}

# Parse CLI argument --release-type
while [[ $# -gt 0 ]]; do
  key="$1"
  case $key in
    --release-type)
      RELEASE_TYPE="$2"
      shift
      shift
      ;;
    *)
      shift
      ;;
  esac
done


SCRIPTS_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd );
VERSION=$("$SCRIPTS_DIR/gen-version.sh");

NEWEST_VERSION="0.0.0"
for tag in $(git tag); do
  CURRENT_VER="0.0.0";
  if [[ $tag == "v$VERSION" ]]; then
    echo "Version $VERSION already exists as a tag"
    exit 1
  fi

  # Parse tag name to semantic version
  if [[ $tag =~ ^v([0-9]+)\.([0-9]+)\.([0-9]+)(.+)?$ ]]; then
    major=${BASH_REMATCH[1]}
    minor=${BASH_REMATCH[2]}
    patch=${BASH_REMATCH[3]}
    type=${BASH_REMATCH[4]}
    CURRENT_VER="$major.$minor.$patch$type"

    compare_versions $CURRENT_VER $NEWEST_VERSION;
    CMP_RES=$?;
    if [[ $CMP_RES == 0 ]]; then
      NEWEST_VERSION=$CURRENT_VER
    fi
    
  fi
done

compare_versions $VERSION $NEWEST_VERSION;
if [[ $? == 1 ]]; then
  echo "Version $VERSION is less than the newest tag $NEWEST_VERSION. Is current branch up to date?"
  exit 1
fi

echo "Last version: $NEWEST_VERSION, new version: $VERSION";
read -p "Do you want to continue updating Cargo.toml? (y/n): " answer
if [[ $answer == "y" ]]; then
  sed -i "s/^version = .*/version = \"$VERSION\"/" $(realpath "$SCRIPTS_DIR/../Cargo.toml")
  echo "Cargo.toml updated successfully"
else
  exit 0
fi

read -p "Add changes to CHANGELOG.md? (y/n): " answer
if [[ $answer == "y" ]]; then  
  printf "\n## Version ${VERSION}\n" >> $(realpath "$SCRIPTS_DIR/../CHANGELOG.md")
  git log --abbrev-commit --pretty=format:"- %s (%h)" "v$NEWEST_VERSION"..HEAD >> "$SCRIPTS_DIR/../CHANGELOG.md"
  echo "CHANGELOG.md updated successfully"
fi

echo "Operations to be performed:"
echo -e "- Will add to git:\n\tCargo.toml\n\tCargo.lock\n\tCHANGELOG.md"
echo "- Will commit with message: \"Release version $VERSION\"";
echo "- Will tag the commit with \"v$VERSION\""
read -p "Confirm operation? (y/n)" answer
if [[ $answer == "y" ]]; then
  git add $(realpath "$SCRIPTS_DIR/../Cargo.toml") $(realpath "$SCRIPTS_DIR/../Cargo.lock") $(realpath "$SCRIPTS_DIR/../CHANGELOG.md")
  git commit -m "Release version $VERSION"
  git tag "v$VERSION"
  echo "Changes committed successfully"
fi

read -p "Do you want to push to origin master? (y/n): " answer
if [[ $answer == "y" ]]; then
  git push origin master
  git push origin "v$VERSION"
  echo "Changes pushed successfully"
fi

read -p "Do you want to build the packages? (y/n): " answer
if [[ $answer == "y" ]]; then
  cargo build --release
  exec "$SCRIPTS_DIR/package-as-deb.sh"
fi

exit 0