# KLEIN

Distributed load balancer. 

Load balancer: Rust

Backend Server: Python3



## Endpoints

### `./add`

The `add` endpoint allows you to add new endpoints to the server.

The format of  a request is 

```json
{"n": 1,"hostnames": ["names"]}
```

Example of a cURL request

```shell
curl "http://localhost:5001/add" -X POST  -H "Content-Type: application/json" -d '{"n":2,"hostnames":["big","boy"]}' 
```

(this assumes that the load balancer is at port `5001` )

The above request will start a new docker container on 
an unspecified port running the backend service which will be 
added to the servers the load balancer will be sending requests.


### `./rm`

Remove a backend server from one of the servers 

This will close a docker service by executing the following command

```shell
docker rm -f --{name}
```

Format of a request is 

```json
{"n": 1,"hostnames": ["names"]}
```

Example of a cURL request

```shell
curl "http://localhost:5001/rm" -X POST  -H "Content-Type: application/json" -d '{"n":2,"hostnames":["big","boy"]}' 
```

