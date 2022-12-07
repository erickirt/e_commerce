#include "utils.h"
#include "base64.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"
//#include "rpc/core.h"

#define  MAX_BYTES_RESP_BODY  250

static void api__dealloc_req_hashmap (app_middleware_node_t *node) {
    char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    if(_res_id_encoded) {
        free(_res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "_res_id_encoded", (void *)NULL);
    }
}

static void  api__complete_multipart_upload__db_async_err(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    h2o_send_error_503(req, "server temporarily unavailable", "", H2O_SEND_ERROR_KEEP_HEADERS);
    api__dealloc_req_hashmap(node);
    app_run_next_middleware(self, req, node);
} // end of api__complete_multipart_upload__db_async_err


static void  api__complete_multipart_upload__db_write_done(db_query_t *target, db_query_result_t *rs)
{
    assert(rs->_final);
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
        uint32_t curr_req_seq = (uint32_t)app_fetch_from_hashmap(node->data, "req_seq");
#pragma GCC diagnostic pop
        char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
        size_t res_id_len = 0;
        unsigned char *resource_id = base64_decode((const unsigned char *)_res_id_encoded,
                strlen(_res_id_encoded), &res_id_len);
        json_t *res_body = json_object();
        json_object_set_new(res_body, "resource_id", json_string((const char *)resource_id));
        json_object_set_new(res_body, "req_seq",  json_integer(curr_req_seq));
        json_object_set_new(res_body, "usr_id" ,  json_integer(curr_usr_id));
        char body_raw[MAX_BYTES_RESP_BODY] = {0};
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
        req->res.status = (uint32_t) target->cfg.usr_data.entry[3];
#pragma GCC diagnostic pop
        h2o_send_inline(req, body_raw, nwrite);
        json_decref(res_body);
        free(resource_id);
    }
    api__dealloc_req_hashmap(node);
    app_run_next_middleware(self, req, node);
} // end of api__complete_multipart_upload__db_write_done


#define SQL_PATTERN__UPLOAD_REQ__SET_COMMITTED_TIME \
    "UPDATE `upload_request` SET `time_committed` = '%s' WHERE `req_id` = x'%08x' AND `usr_id` = %u;"

static int api__complete_upload__resource_id_exist (RESTAPI_HANDLER_ARGS(self, req),
        app_middleware_node_t *node, uint32_t last_req_seq, uint32_t resource_owner_id )
{
#define SQL_PATTERN  \
      "BEGIN NOT ATOMIC" \
      "  START TRANSACTION;" \
      "    UPDATE `upload_request` SET `time_committed`=NULL WHERE `req_id`=x'%08x' AND `usr_id`=%u;" \
      "    EXECUTE IMMEDIATE 'UPDATE `uploaded_file` SET `usr_id`=?, `last_upld_req`=?, `last_update`=?  WHERE `id`=?'" \
      "        USING %u,x'%08x','%s',FROM_BASE64('%s'); " \
      "    " SQL_PATTERN__UPLOAD_REQ__SET_COMMITTED_TIME \
      "  COMMIT;" \
      "END;"
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
    if(curr_usr_id == resource_owner_id || resource_owner_id == 0) {
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
        uint32_t curr_req_seq = (uint32_t)app_fetch_from_hashmap(node->data, "req_seq");
#pragma GCC diagnostic pop
        char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
        size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(_res_id_encoded) + USR_ID_STR_SIZE*3 +
                  (DATETIME_STR_SIZE - 1)*2 + UPLOAD_INT2HEX_SIZE(curr_req_seq)*3;
        char raw_sql[raw_sql_sz];
        char curr_time_str[DATETIME_STR_SIZE] = {0};
        {
            time_t now_time = time(NULL);
            struct tm *brokendown = localtime(&now_time);
            strftime(&curr_time_str[0], DATETIME_STR_SIZE, "%F %T", brokendown); // ISO8601 date format
        }
        memset(&raw_sql[0], 0x0, sizeof(char) *  raw_sql_sz);
        snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, last_req_seq, resource_owner_id, curr_usr_id, curr_req_seq,
                &curr_time_str[0], _res_id_encoded, &curr_time_str[0], curr_req_seq, curr_usr_id);
#define NUM_USR_ARGS 4
        void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node, (void *)200};
        db_query_cfg_t  cfg = {
            .statements = {.entry = &raw_sql[0], .num_rs = 1},
            .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
            .pool = app_db_pool_get_pool("db_server_1"),
            .loop = req->conn->ctx->loop,
            .callbacks = {
                .result_rdy  = api__complete_multipart_upload__db_write_done,
                .row_fetched = app_db_async_dummy_cb,
                .result_free = app_db_async_dummy_cb,
                .error =  api__complete_multipart_upload__db_async_err,
            }
        };
        if(app_db_query_start(&cfg) != DBA_RESULT_OK) {
            db_query_t  fake_q = {.cfg = {.usr_data = {.entry = (void **)&db_async_usr_data[0], .len=NUM_USR_ARGS}}};
            api__complete_multipart_upload__db_async_err(&fake_q, NULL);
        }
#undef NUM_USR_ARGS
    } else {
        char body_raw[] = "{\"resource_id\":\"NOT allowed to use the ID\"}";
        req->res.status = 403;
        h2o_send_inline(req, body_raw, strlen(body_raw));
        api__dealloc_req_hashmap(node);
        app_run_next_middleware(self, req, node);
    }
    return 0;
#undef SQL_PATTERN
} // end of api__complete_upload__resource_id_exist


static int api__complete_upload__resource_id_notexist(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) 
{
#define SQL_PATTERN  \
    "BEGIN NOT ATOMIC" \
    "  START TRANSACTION;" \
    "    EXECUTE IMMEDIATE 'INSERT INTO `uploaded_file`(`id`,`usr_id`,`last_upld_req`,`last_update`) VALUES (?,?,?,?)'" \
    "       USING FROM_BASE64('%s'),%u,x'%08x','%s';" \
    "    " SQL_PATTERN__UPLOAD_REQ__SET_COMMITTED_TIME \
    "  COMMIT;" \
    "END;"
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint32_t curr_req_seq = (uint32_t)app_fetch_from_hashmap(node->data, "req_seq");
#pragma GCC diagnostic pop
    char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(_res_id_encoded) + USR_ID_STR_SIZE*2 +
              (DATETIME_STR_SIZE - 1)*2 + UPLOAD_INT2HEX_SIZE(curr_req_seq)*2;
    char raw_sql[raw_sql_sz];
    char curr_time_str[DATETIME_STR_SIZE] = {0};
    {
        time_t now_time = time(NULL);
        struct tm *brokendown = localtime(&now_time);
        strftime(&curr_time_str[0], DATETIME_STR_SIZE, "%F %T", brokendown); // ISO8601 date format
    }
    memset(&raw_sql[0], 0x0, sizeof(char) *  raw_sql_sz);
    snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, _res_id_encoded, curr_usr_id, curr_req_seq,
            &curr_time_str[0], &curr_time_str[0], curr_req_seq, curr_usr_id);
#define NUM_USR_ARGS 4
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node, (void *)201};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = api__complete_multipart_upload__db_write_done,
            .row_fetched = app_db_async_dummy_cb,
            .result_free = app_db_async_dummy_cb,
            .error =  api__complete_multipart_upload__db_async_err,
        }
    };
    if(app_db_query_start(&cfg) != DBA_RESULT_OK) {
        db_query_t  fake_q = {.cfg = {.usr_data = {.entry = (void **)&db_async_usr_data[0], .len=NUM_USR_ARGS}}};
        api__complete_multipart_upload__db_async_err(&fake_q, NULL);
    }
    return 0;
#undef NUM_USR_ARGS
#undef SQL_PATTERN
} // end of api__complete_upload__resource_id_notexist


static void _api_complete_upload__check_resource_id_done (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    if(result->flag.error) {
        void *args[3] = {(void *)req, (void *)hdlr, (void *)node};
        db_query_t  fake_q = {.cfg = {.usr_data = {.entry = (void **)&args[0], .len=3}}};
        api__complete_multipart_upload__db_async_err(&fake_q, NULL);
    } else if (result->flag.res_id_exists) {
        api__complete_upload__resource_id_exist (hdlr, req, node, result->upld_req, result->owner_usr_id);
    } else {
        api__complete_upload__resource_id_notexist (hdlr, req, node);
    }
} // end of  _api_complete_upload__check_resource_id_done


static void api__complete_multipart_upload__validate_filechunks__rs_free(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *)     target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint32_t parts_max  = (uint32_t) target->cfg.usr_data.entry[3];
    uint32_t parts_min  = (uint32_t) target->cfg.usr_data.entry[4];
    uint32_t parts_cnt  = (uint32_t) target->cfg.usr_data.entry[5];
#pragma GCC diagnostic pop
    uint8_t err = (parts_max == 0 || parts_min == 0 || parts_cnt == 0) || (parts_min != 1)
        || (parts_max != parts_cnt);
    if(err) {
        char body_raw[] = "{\"req_seq\":\"part numbers of file chunks are not adjacent\"}";
        req->res.status = 400;
        h2o_send_inline(req, body_raw, strlen(body_raw));
        api__dealloc_req_hashmap(node);
        app_run_next_middleware(self, req, node);
    } else {
        char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
        void *usr_args[3] = {req, self, node};
        aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=3}, .resource_id=(char *)_res_id_encoded,
                .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                .callback=_api_complete_upload__check_resource_id_done };
        err = app_acl_verify_resource_id (&cfg);
        if(err) {
            void *args[3] = {(void *)req, (void *)self, (void *)node};
            db_query_t  fake_q = {.cfg = {.usr_data = {.entry = (void **)&args[0], .len=3}}};
            api__complete_multipart_upload__db_async_err(&fake_q, NULL);
        }
    }
} // end of api__complete_multipart_upload__validate_filechunks__rs_free

static void api__complete_multipart_upload__validate_filechunks__row_fetch(db_query_t *target, db_query_result_t *rs)
{
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];
    uint32_t parts_max  = (uint32_t) strtoul(row->values[0], NULL, 10);
    uint32_t parts_min  = (uint32_t) strtoul(row->values[1], NULL, 10);
    uint32_t parts_cnt  = (uint32_t) strtoul(row->values[2], NULL, 10);
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
    target->cfg.usr_data.entry[3] = (void *) parts_max;
    target->cfg.usr_data.entry[4] = (void *) parts_min;
    target->cfg.usr_data.entry[5] = (void *) parts_cnt;
#pragma GCC diagnostic pop
} // end of api__complete_multipart_upload__validate_filechunks__row_fetch


static DBA_RES_CODE  api__complete_multipart_upload__validate_filechunks(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    int req_seq = (int)app_fetch_from_hashmap(node->data, "req_seq");
#pragma GCC diagnostic pop
#define SQL_PATTERN "SELECT MAX(`part`), MIN(`part`), COUNT(`part`) FROM `upload_filechunk` " \
        " WHERE `usr_id` = %u AND `req_id` = x'%08x' GROUP BY `req_id`;"
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(req_seq);
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, usr_id, req_seq);
    assert(nwrite_sql < raw_sql_sz);
#undef SQL_PATTERN
#define  NUM_USR_ARGS  6
    void *usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node,
            (void *)0, (void *)0, (void *)0 };
    db_query_cfg_t  cfg = {
        .statements = {.entry = raw_sql, .num_rs = 1},
        .usr_data = {.entry = (void **)&usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = app_db_async_dummy_cb,
            .row_fetched = api__complete_multipart_upload__validate_filechunks__row_fetch,
            .result_free = api__complete_multipart_upload__validate_filechunks__rs_free,
            .error = api__complete_multipart_upload__db_async_err,
        }
    };
#undef NUM_USR_ARGS
    return app_db_query_start(&cfg);
} // end of api__complete_multipart_upload__validate_filechunks
  
static int api__complete_multipart_upload__validate_reqseq_success(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) 
{
    DBA_RES_CODE result = api__complete_multipart_upload__validate_filechunks(self, req, node);
    if(result != DBA_RESULT_OK) {
        void *args[3] = {(void *)req, (void *)self, (void *)node};
        db_query_t  fake_q = {.cfg = {.usr_data = {.entry = (void **)&args[0], .len=3}}};
        api__complete_multipart_upload__db_async_err(&fake_q, NULL);
    }
    return 0;
} // end of api__complete_multipart_upload__validate_reqseq_success

static int api__complete_multipart_upload__validate_reqseq_failure(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) 
{
    char body_raw[] = "{\"req_seq\":\"request not exists\"}";
    req->res.status = 400;
    h2o_send_inline(req, body_raw, strlen(body_raw));
    api__dealloc_req_hashmap(node);
    app_run_next_middleware(self, req, node);
    return 0;
}


// TODO:another API endpoint for checking status of each upload request that hasn't expired yet
RESTAPI_ENDPOINT_HANDLER(complete_multipart_upload, PATCH, self, req)
{
    json_error_t  j_err = {0};
    const char *json_decode_err = NULL;
    const char *res_id_err  = NULL;
    const char *req_seq_err = NULL;
    req->res.status = 200;
    json_t *req_body = json_loadb((const char *)req->entity.base, req->entity.len, JSON_REJECT_DUPLICATES, &j_err);
    if(j_err.line >= 0 || j_err.column >= 0) {
        json_decode_err = "parsing error on request body";
        req->res.status = 400;
    }
    const char *resource_id = json_string_value(json_object_get(req_body, "resource_id"));
    uint32_t req_seq = (uint32_t) json_integer_value(json_object_get(req_body, "req_seq"));
    if(resource_id) {
        int err = app_verify_printable_string(resource_id, APP_RESOURCE_ID_SIZE);
        if(err) { // TODO, consider invalid characters in SQL string literal for each specific database
            res_id_err = "invalid format";
            req->res.status = 400;
        }
    } else {
        res_id_err = "missing resource ID";
        req->res.status = 400;
    }
    if(req_seq == 0) {
        req_seq_err = "missing upload request";
        req->res.status = 400;
    }
    if(req->res.status != 200) {
        req->res.reason = "invalid ID";
        json_t *res_body = json_object();
        if(json_decode_err)
            json_object_set_new(res_body, "message", json_string(json_decode_err));
        if(res_id_err)
            json_object_set_new(res_body, "resource_id", json_string(res_id_err));
        if(req_seq_err)
            json_object_set_new(res_body, "req_seq", json_string(req_seq_err));
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
        json_decref(res_body);
        app_run_next_middleware(self, req, node);
    } else {
        size_t out_len = 0;
        unsigned char *_res_id_encoded = base64_encode((const unsigned char *)resource_id,
                strlen(resource_id), &out_len);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)_res_id_encoded);
        app_save_int_to_hashmap(node->data, "req_seq", req_seq);
        DBA_RES_CODE db_result = app_validate_uncommitted_upld_req (
                self, req, node, "upload_request", api__complete_multipart_upload__db_async_err,
                api__complete_multipart_upload__validate_reqseq_success,
                api__complete_multipart_upload__validate_reqseq_failure
            );
        if(db_result != DBA_RESULT_OK) {
            h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
            app_run_next_middleware(self, req, node);
        }
    }
    if(req_body)
        json_decref(req_body);
    return 0;
} // end of complete_multipart_upload()

#undef  MAX_BYTES_MSG_BODY
