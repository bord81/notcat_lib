#ifndef NOTCAT_CLIENT_H
#define NOTCAT_CLIENT_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NotCatClientHandle NotCatClientHandle;

NotCatClientHandle* notcat_connect(const char* path);

int notcat_log(NotCatClientHandle* client, const char* message);

int notcat_close(NotCatClientHandle* client);

#ifdef __cplusplus
}
#endif

#endif // NOTCAT_CLIENT_H