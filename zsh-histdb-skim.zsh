XDG_BIN_PATH=${XDG_DATA_HOME:-$HOME/.local/share}/zsh-histdb-skim/
BIN_DIR=${HISTDB_SKIM_PATH:-${XDG_BIN_PATH}}
BIN_PATH=${BIN_DIR}/zsh-histdb-skim

histdb-skim-get-os(){
  UNAME_STR=`uname -a`
  if [[ ( $UNAME_STR =~ '.*Darwin.*' ) && ( $UNAME_STR =~ '.*x86_64.*') ]]; then
    echo -n "darwin-x64"
  fi
  if [[ ( $UNAME_STR =~ '.*Darwin.*' ) && ( $UNAME_STR =~ '.*arm64.*') ]]; then
    echo -n "darwin-x64"
  fi
  if [[ ( $UNAME_STR =~ '.*Linux.*' ) && ( $UNAME_STR =~ '.*x86_64.*') ]]; then
    echo -n "linux-x64"
  fi
}

histdb-skim-get-latest-version(){
  curl -s "https://api.github.com/repos/jordan-turbofish/zsh-histdb-skim/releases/latest" | grep tag_name | cut -f 4 -d '"'
}

histdb-skim-download(){
  local HISTDB_SKIM_VERSION=$(histdb-skim-get-latest-version)
  if [[ -z $(histdb-skim-get-os) ]]; then
    echo "Could not find prebuild executable"
    echo "Sorry, you have to do it yourself"
  elif [[ -z "$HISTDB_SKIM_VERSION" ]]; then
    echo "Could not find prebuild executable"
    echo "Sorry, you have to do it yourself"
  else
    echo "Downloading binary"
    mkdir -p ${BIN_DIR}
    curl -s -JL https://github.com/jordan-turbofish/zsh-histdb-skim/releases/download/${HISTDB_SKIM_VERSION}/zsh-histdb-skim-$(histdb-skim-get-os) -o ${BIN_PATH}
    chmod +x ${BIN_PATH}
  fi
}

histdb-skim-ensure () {
  local HISTDB_SKIM_VERSION=$(histdb-skim-get-latest-version)
  if [[ -z "$HISTDB_SKIM_VERSION" ]]; then
    echo "Could not find prebuild executable"
    echo "Sorry, you have to do it yourself"
  elif [[ ! -f ${BIN_PATH} || $(${BIN_PATH} --version) != ${HISTDB_SKIM_VERSION} ]]; then
    histdb-skim-download
  fi
}

histdb-skim-widget() {
  origquery=${BUFFER}
  output=$( \
    HISTDB_HOST=${HISTDB_HOST:-"'$(sql_escape ${HOST})'"} \
    HISTDB_SESSION=$HISTDB_SESSION \
    HISTDB_FILE=$HISTDB_FILE \
    ${BIN_PATH} "$origquery"\
  )

  if [ $? -eq 0 ]; then
    BUFFER=$output
  else
    BUFFER=$origquery
  fi

  CURSOR=$#BUFFER
  zle redisplay
}

histdb-skim-ensure

zle     -N   histdb-skim-widget
bindkey '^R' histdb-skim-widget
