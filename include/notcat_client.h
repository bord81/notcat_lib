#ifndef NOTCAT_CLIENT_H
#define NOTCAT_CLIENT_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NotCatClientHandle NotCatClientHandle;

typedef enum {
    LOG_VERBOSE = 0,
    LOG_DEBUG   = 1,
    LOG_INFO    = 2,
    LOG_WARN    = 3,
    LOG_ERROR   = 4,
} notcat_log_priority_t;

NotCatClientHandle* notcat_connect(const char* path);

int notcat_log(NotCatClientHandle* client, int priority, const char* message);

int notcat_close(NotCatClientHandle* client);

#ifdef __cplusplus
}
#endif

#endif // NOTCAT_CLIENT_H