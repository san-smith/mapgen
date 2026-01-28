# Земля
cargo run --bin mapgen-cli -- --config examples/earth_like.toml --output output/earth

# Суперконтинент
cargo run --bin mapgen-cli -- --config examples/supercontinent.toml --output output/pangea

# Архипелаг
cargo run --bin mapgen-cli -- --config examples/archipelago.toml --output output/islands

# Средиземноморье
cargo run --bin mapgen-cli -- --config examples/mediterranean.toml --output output/med

# Ледниковый период
cargo run --bin mapgen-cli -- --config examples/ice_age_earth.toml --output output/ice_age