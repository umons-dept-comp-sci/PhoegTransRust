1 run computation using run-par.sh

2 send data to server using scp

directory=""
location="compute-results/"
server=""

scp -r $directory $server:$location

3 on the server (to avoid using local disc), group all files in on file:

file=""
for n in $(seq 1 10); do
    cat $file-$n.csv >> $file.csv
done

4 input files in the database

sql:
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

When running a query from a sql file :

run-mail psql -e -f file.sql database
The -e option prints the query (so we have it in the mail too)

Suggested configurations (obtained with irace) :
1  -b  1664799  -s  546791613   -t  3  -c  188471
2  -b  6689800  -s  2774808391  -t  3  -c  107216
3  -b  8103867  -s  1094573238  -t  3  -c  137106
4  -b  4365596  -s  816177529   -t  1  -c  44432
