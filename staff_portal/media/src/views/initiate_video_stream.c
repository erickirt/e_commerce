#include "app_cfg.h"
#include "utils.h"
#include "base64.h"
#include "views.h"
#include "models/query.h"
#include "storage/cfg_parser.h"
#include "transcoder/file_processor.h"

#define   ASA_USRARG_INDEX__AFTP         ATFP_INDEX__IN_ASA_USRARG
#define   ASA_USRARG_INDEX__ASAOBJ_MAP   ASAMAP_INDEX__IN_ASA_USRARG
#define   NUM_USRARGS_ASA_SRC            (ASA_USRARG_INDEX__ASAOBJ_MAP + 1)
#define   ASA_SRC_RD_BUF_SZ                     512
#define   APP_UPDATE_INTERVAL_SECS_MST_PLIST    30.0f
#define   APP_UPDATE_INTERVAL_SECS_KEYFILE      60.0f

static  void  _api_initiate_video_stream__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *qparams, json_t *res_body)
{
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    size_t  nb_required = json_dumpb(res_body, NULL, 0, 0);
    if(nb_required > 0) {
        char  body[nb_required];
        size_t  nwrite = json_dumpb(res_body, &body[0], nb_required, JSON_COMPACT);
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, nwrite);
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    json_decref(res_body);
    json_decref(qparams);
    char *res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    if(res_id_encoded) {
        free(res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)NULL);
    }
    // TODO, dealloc jwt if created for ACL check
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_initiate_video_stream__deinit_primitives


static void _api_atfp_init_stream__done_cb(atfp_t *processor)
{
    json_t  *resp_body = NULL;
    json_t  *err_info = processor->data.error;
    json_t  *spec = processor->data.spec;
    json_t  *qparams  = spec;
    h2o_req_t *req = (h2o_req_t *) json_integer_value(json_object_get(spec, "_http_req"));
    h2o_handler_t *hdlr = (h2o_handler_t *) json_integer_value(json_object_get(spec, "_http_handler"));
    app_middleware_node_t *node = (app_middleware_node_t *) json_integer_value(
            json_object_get(spec, "_middleware_node"));
    if(json_object_size(err_info) == 0) {
        json_decref(err_info);
        resp_body = json_object_get(spec, "return_data");
    } else {
        resp_body = err_info;
    }
    req->res.status = (int) json_integer_value(json_object_get(spec, "http_resp_code"));
    processor->data.error = NULL;
    processor->data.spec = NULL;
    _api_initiate_video_stream__deinit_primitives (req, hdlr, node, qparams, resp_body);
} // end of _api_atfp_init_stream__done_cb


static void api__initiate_video_stream__db_async_err (db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = target->cfg.usr_data.entry[0];
    h2o_handler_t *hdlr = target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = target->cfg.usr_data.entry[2];
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *qparams  = app_fetch_from_hashmap(node->data, "qparams");
    json_object_set_new(err_info, "id", json_string("error happended during validation"));
    req->res.status = 500;
    _api_initiate_video_stream__deinit_primitives (req, hdlr, node, qparams, err_info);
} // end of  api__initiate_video_stream__db_async_err


static int api__initiate_video_stream__resource_id_exist (h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node) 
{
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint32_t  last_upld_seq = (uint32_t) app_fetch_from_hashmap(node->data, "last_upld_req");
    uint32_t  res_owner_id  = (uint32_t) app_fetch_from_hashmap(node->data, "resource_owner_id");
#pragma GCC diagnostic pop
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *qparams  = app_fetch_from_hashmap(node->data, "qparams");
    const char *label = "hls"; // TODO, store stream types to database once there are more to support
    const char *storage_alias = "localfs";
    atfp_t  *processor = app_transcoder_file_processor(label);
    if(!processor) {
        req->res.status = 500;
        goto done;
    }
    asa_cfg_t *storage = app_storage_cfg_lookup(storage_alias);
    asa_op_base_cfg_t *asa_src = app_storage__init_asaobj_helper (storage,
            NUM_USRARGS_ASA_SRC, ASA_SRC_RD_BUF_SZ, 0);
    if(!asa_src) {
        req->res.status = 500;
        goto done;
    } {
        app_cfg_t *acfg = app_get_global_cfg();
        json_t *hostinfo = json_object(), *qp_labels = json_object(), *update_interval = json_object();
        json_object_set_new(hostinfo, "domain", json_string(req->authority.base));  // h2o_iovec_t, domain name + port
        json_object_set_new(hostinfo, "path", json_string("/video/playback/seek")); // TODO, parameterize
        json_object_set_new(qp_labels, "resource_id", json_string("doc_id"));
        json_object_set_new(qp_labels, "version", json_string("doc_ver"));
        json_object_set_new(qp_labels, "detail", json_string("detail"));
        json_object_set_new(update_interval, "playlist",  json_real(APP_UPDATE_INTERVAL_SECS_MST_PLIST));
        json_object_set_new(update_interval, "keyfile",   json_real(APP_UPDATE_INTERVAL_SECS_KEYFILE));
        json_object_set_new(qparams, "host", hostinfo);
        json_object_set_new(qparams, "query_param_label", qp_labels);
        json_object_set_new(qparams, "update_interval",  update_interval);
    }
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    // TODO, current implementation assumes the app server runs on hardware with  32-bit or 64-bit address mode
    // , if the sserver provides more computing cability e.g. 128-bit address mode, then the  code below has
    // to be adjusted accroding to max number of bits applied to address
    json_object_set_new(qparams, "_http_req",     json_integer((uint64_t)req)); // for backup purpose
    json_object_set_new(qparams, "_http_handler", json_integer((uint64_t)hdlr)); 
    json_object_set_new(qparams, "_middleware_node", json_integer((uint64_t)node));
    json_object_set_new(qparams, "loop", json_integer((uint64_t)req->conn->ctx->loop));
#pragma GCC diagnostic pop
    json_object_set_new(qparams, "db_alias", json_string("db_server_1"));
    json_object_set_new(qparams, "storage_alias", json_string(storage->alias));
    asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP] = processor;
    asa_src->deinit = (void (*)(asa_op_base_cfg_t *)) free;
    if(!strcmp(storage->alias, "localfs"))
        ((asa_op_localfs_cfg_t *)asa_src)->loop = req->conn->ctx->loop; // TODO
    processor->data = (atfp_data_t) {.error=err_info, .spec=qparams, .callback=_api_atfp_init_stream__done_cb,
          .usr_id=res_owner_id, .upld_req_id=last_upld_seq, .storage={.handle=asa_src}};
    processor->ops->init(processor);
    if(json_object_size(err_info) > 0) // 4xx or 5xx
        req->res.status = (int) json_integer_value(json_object_get(qparams, "http_resp_code"));
done:
    if(json_object_size(err_info) > 0)
        _api_initiate_video_stream__deinit_primitives (req, hdlr, node, qparams, err_info);
    return 0;
} // end of  api__initiate_video_stream__resource_id_exist


static int api__initiate_video_stream__resource_id_notexist (h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node) 
{
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *qparams  = app_fetch_from_hashmap(node->data, "qparams");
    json_object_set_new(err_info, "id", json_string("not exists"));
    req->res.status = 404;
    _api_initiate_video_stream__deinit_primitives (req, hdlr, node, qparams, err_info);
    return 0;
} // end of  api__initiate_video_stream__resource_id_notexist


static  int  app__validate_file_acl(const char *resource_id, h2o_req_t *req, app_middleware_node_t *node,
        void **usr_args, size_t num_usr_args)
{
    int err = 0;
    // TODO
    // * check whether json file exists (users ACL), if not, create one; or if it exists, then still refresh the 
    //   content if the last update is before certain time llmit.
    // * refresh users ACL from database to local api server (saved in temp buffer)
    //   (may improve the flow by sending message queue everytime when user ACL has been updaated)
    // * examine user ACL, if it is NOT public, authenticate client JWT, then check the auth user
    // has permission to watch this video.
    return  err;
}


RESTAPI_ENDPOINT_HANDLER(initiate_video_stream, POST, self, req)
{
    int  err = 0;
    json_t *err_info = json_object();
    json_t *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "id"));
    size_t  res_id_sz = strlen(resource_id);
    if(res_id_sz > APP_RESOURCE_ID_SIZE) {
        json_object_set_new(err_info, "id", json_string("exceeding max limit"));
        req->res.status = 400;
    }
    if(json_object_size(err_info) == 0) {
        err = app_verify_printable_string(resource_id, res_id_sz);
        if(err) {
            json_object_set_new(err_info, "id", json_string("contains non-printable charater"));
            req->res.status = 400;
        }
    }
    if(json_object_size(err_info) == 0) {
#define  VALIDATE_FILE_ACL__USR_ARGS_SZ  3
        void *usr_args[VALIDATE_FILE_ACL__USR_ARGS_SZ] = {self, qparams, err_info};
        size_t num_usr_args = VALIDATE_FILE_ACL__USR_ARGS_SZ;
        err  = app__validate_file_acl(resource_id, req, node, (void **)usr_args, num_usr_args);
        if(err) {
            json_object_set_new(err_info, "id", json_string("failed to validate file access control on the user"));
            req->res.status = 403;
        }
#undef   VALIDATE_FILE_ACL__USR_ARGS_SZ
    }
    if(json_object_size(err_info) == 0) {
        size_t out_len = 0;
        unsigned char *res_id_encoded = base64_encode((const unsigned char *)resource_id,
                 res_id_sz, &out_len);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info);
        app_save_ptr_to_hashmap(node->data, "qparams", (void *)qparams);
        DBA_RES_CODE  result = app_verify_existence_resource_id (
            self, req, node, api__initiate_video_stream__db_async_err,
            api__initiate_video_stream__resource_id_exist,
            api__initiate_video_stream__resource_id_notexist
        );
        if(result != DBA_RESULT_OK) {
            json_object_set_new(err_info, "model", json_string("failed to validate resource ID"));
            req->res.status = 503;
        }
    }
    if(json_object_size(err_info) > 0)
        _api_initiate_video_stream__deinit_primitives (req, self, node, qparams, err_info);
    return 0;
} // end of initiate_video_stream

