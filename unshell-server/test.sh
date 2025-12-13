# curl -w '\n' \
#     -H 'Content-Type: application/json' \
#     -d '{"username":"foo","password":"bar"}' \
#     http://localhost:3000/api/auth



curl -s \
     -w '\n' \
     -H 'Content-Type: application/json' \
     -H 'Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3NjQ2NjU1ODMsImlhdCI6MTc2NDYyMjM4MywiZW1haWwiOiJmb28ifQ.NeStaGwWBGS825rF11TOH_e79RWEL_2o3SY9jZ5CX20' \
     -d "jwbrjwbremnwebrnmwemnrbnmwerbnmwer" \
     http://localhost:3000/api/test


curl -s \
     -w '\n' \
     -H 'Content-Type: application/json' \
     -H 'Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3NjQ2NjU1ODMsImlhdCI6MTc2NDYyMjM4MywiZW1haWwiOiJmb28ifQ.NeStaGwWBGS825rF11TOH_e79RWEL_2o3SY9jZ5CX20' \
     http://localhost:3000/api/test/test2
