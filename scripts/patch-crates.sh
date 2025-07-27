# source this file

update_Alembic_dependencies() {
  declare project_root="$1"
  declare Alembic_ver="$2"
  declare tomls=()
  while IFS='' read -r line; do tomls+=("$line"); done < <(find "$project_root" -name Cargo.toml)

  sed -i -e "s#\(Alembic-program = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-program = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-program-test = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-program-test = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-sdk = \"\).*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-sdk = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-client = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-client = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-cli-config = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-cli-config = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-clap-utils = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-clap-utils = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-account-decoder = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-account-decoder = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-faucet = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-faucet = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-zk-token-sdk = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
  sed -i -e "s#\(Alembic-zk-token-sdk = { version = \"\)[^\"]*\(\"\)#\1=$Alembic_ver\2#g" "${tomls[@]}" || return $?
}

patch_crates_io_Alembic() {
  declare Cargo_toml="$1"
  declare Alembic_dir="$2"
  cat >> "$Cargo_toml" <<EOF
[patch.crates-io]
EOF
patch_crates_io_Alembic_no_header "$Cargo_toml" "$Alembic_dir"
}

patch_crates_io_Alembic_no_header() {
  declare Cargo_toml="$1"
  declare Alembic_dir="$2"
  cat >> "$Cargo_toml" <<EOF
Alembic-account-decoder = { path = "$Alembic_dir/account-decoder" }
Alembic-clap-utils = { path = "$Alembic_dir/clap-utils" }
Alembic-client = { path = "$Alembic_dir/client" }
Alembic-cli-config = { path = "$Alembic_dir/cli-config" }
Alembic-program = { path = "$Alembic_dir/sdk/program" }
Alembic-program-test = { path = "$Alembic_dir/program-test" }
Alembic-sdk = { path = "$Alembic_dir/sdk" }
Alembic-faucet = { path = "$Alembic_dir/faucet" }
Alembic-zk-token-sdk = { path = "$Alembic_dir/zk-token-sdk" }
EOF
}
