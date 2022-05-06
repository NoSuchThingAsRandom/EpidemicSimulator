export RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
export RAYON_NUM_THREADS=40
export RUST_BACKTRACE=full
version="v1.5"
machine="workstation"
area="1946157112TYPE299"
full_path="statistics_results/"$machine"/"$version"/"$area"/"
for index in {1..1}
do
  output=$full_path$index"/"
  mkdir -p $output
  echo "Saving results to: '"$output"'"
  cargo run --release -- $area --directory=data --grid-size=250000 --use-cache --simulate --output_name=$output 2>&1 | tee $output"log.log"
done

#2013265923TYPE299
#1946157112TYPE299
