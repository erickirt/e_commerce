#ifndef MEDIA__STORAGE_CFG_PARSER_H
#define MEDIA__STORAGE_CFG_PARSER_H
#ifdef __cplusplus
extern "C" {
#endif

#include "app.h"

int parse_cfg_storages(json_t *objs, app_cfg_t *app_cfg);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__STORAGE_CFG_PARSER_H
