export RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
export RAYON_NUM_THREADS=40
export RUST_BACKTRACE=full
version="v1.7.2"
machine="workstation"
area="1946157112TYPE299"
full_path="statistics_results/"$machine"/"$version"/"$area"/"
mkdir -p full_path
echo "Saving results to: '"$full_path"'"
cargo run --release -- $area --directory=data --grid-size=250000 --use-cache --simulate --output_name=$full_path 2>&1 | tee $full_path"log.log"
#2013265923TYPE299
#1946157112TYPE299
