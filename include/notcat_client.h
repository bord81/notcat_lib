#ifndef NOTCAT_CLIENT_H
#define NOTCAT_CLIENT_H

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    LOG_VERBOSE = 0,
    LOG_DEBUG   = 1,
    LOG_INFO    = 2,
    LOG_WARN    = 3,
    LOG_ERROR   = 4,
} notcat_log_priority_t;

typedef enum {
    SINK_TYPE_LOCAL_FILE = 1,
    SINK_TYPE_ANDROID_LOGCAT = 2,
} notcat_sink_type_t;

int notcat_init(unsigned char sink_type);

int notcat_log(int priority, const char* message);

int notcat_close();

#ifdef __cplusplus
}
#endif

#endif // NOTCAT_CLIENT_H