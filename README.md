TransProof is a system to compute transformations of graphs in a multi-threaded and efficient way. It will filter out symmetrical results thanks to the computation of the orbits of the automorphism group provided by [nauty]. It uses a custom graph library available [here](https://github.com/umons-dept-comp-sci/PhoegRustGraph).

To use it, simply compile it using cargo:

```
cargo build --release
```

You can then provide the graphs in the graph6 format with one graph per line. The available options are given below.

```
Usage:
    transrust [options] remove <e>
    transrust [options] <transformations>...
    transrust (-h | --help)
    transrust --transfos

Options:
    -h, --help             Show this message.
    -v, --verbose          Shows more information.
    --transfos             Shows a list of available transformations.
    -i, --input <input>    File containing the graph6 signatures. Uses the standard input if '-'.
                           [default: -]
    -o, --output <output>  File where to write the result. Uses the standard output if '-'.
                           [default: -]
    -b, --batch <batch>    Batch size [default: 1000000]
    -s, --buffer <buffer>  Size of the buffer [default: 2000000000]
    -t <threads>           Number of threads to be used for computation. A value of 0 means using
                           as many threads cores on the machine. [default: 0]
    -c <channel>           Size of the buffer to use for each threads (in number of messages). If
                           the size is 0, the buffer is unlimited. Use this if you have memory
                           issues even while setting a smaller output buffer and batch size.
                           [default: 0]
    -a, --append           Does not overwrite output file but appends results instead.
    -f, --filter           Only outputs incorrect transfos.
    --postgres             Format as a csv ready to import in a postgresql table.
```

Note that, the `--filter` option is currently under development and is supposed to be used with a [Redis] server storing the values of the graph invariants.
    
The following describes how to add a new transformation in PHOEG's database.

1. Run the computation using `run-par.sh` if you want to use multiple machines at the same time. This requires the [GNU parallel] command.

2. Send the data to the database server using scp:

```
directory=""
location="compute-results/"
server=""

scp -r $directory $server:$location
```

3. On the server (to avoid using local disk), group all the resulting files in on file:

```
file=""
for n in $(seq 1 10); do
    cat $file-$n.csv >> $file.csv
done
```

4. Import the results in the database:

```
On the database:

create temporary table t (name text,
        first text,
        second text,
        sig text,
        new_order text);
copy t from :file with csv;
create table :new_table as select * from disco_twins with no data;
alter table :new_table add constraint pk_:new_table primary key (sig);
alter table :new_table add constraint fk_:new_table_first foreign key (first)
references graphs(sig);
alter table new_table add constraint fk_:new_table_second foreign key (second)
references graphs(sig);
insert into :new_table select sig,
       first,
       second,
       string_to_array(new_order, ';')::integer[]
from t;
drop table t;
```

When running a query from a sql file:

```
run-mail psql -e -f file.sql database
```
The -e option prints the query (so we have it in the mail too)

Suggested configurations (obtained with irace) :
```
1  -b  1664799  -s  546791613   -t  3  -c  188471
2  -b  6689800  -s  2774808391  -t  3  -c  107216
3  -b  8103867  -s  1094573238  -t  3  -c  137106
4  -b  4365596  -s  816177529   -t  1  -c  44432
```

[nauty]: http://pallini.di.uniroma1.it/
[Redis]: https://redis.io/
[GNU parallel]: https://www.gnu.org/software/parallel/
