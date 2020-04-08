#TODO write documentation !

servers="server1,server2,:" #":" is the local computer

function run_par() {
    parallel --bf ./target/release/transrust -S \
    $servers -a data/g6/notjoin-$2.g6 -j 1 --pipepart \
    ./target/release/transrust -c 24000 $1 > "data/$3/$1-$2.csv"
}

for n in $(seq 1 10); do
    echo "COMPUTING $n"
    run_par isolate_incl $n isolate-incl
done
