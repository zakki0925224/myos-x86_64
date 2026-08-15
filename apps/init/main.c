#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define RC_LUA_PATH "/mnt/initramfs/rc.lua"
#define MAX_SERVICES 16
#define NAME_MAX_LEN 32
#define CMD_MAX_LEN 256

typedef struct {
    char name[NAME_MAX_LEN];
    char cmd[CMD_MAX_LEN];
    int required;
} service_t;

static service_t services[MAX_SERVICES];
static int service_count = 0;

static void load_default_services(void) {
    service_count = 1;
    strncpy(services[0].name, "sh", NAME_MAX_LEN - 1);
    strncpy(services[0].cmd, "/mnt/initramfs/apps/bin/sh /mnt/initramfs/apps/bin", CMD_MAX_LEN - 1);
    services[0].required = 1;
}

static int load_services_from_lua(const char* path) {
    lua_State* L = luaL_newstate();
    if (L == NULL) {
        printf("init: luaL_newstate failed\n");
        return -1;
    }

    luaL_requiref(L, "_G", luaopen_base, 1);
    lua_pop(L, 1);
    luaL_requiref(L, LUA_TABLIBNAME, luaopen_table, 1);
    lua_pop(L, 1);
    luaL_requiref(L, LUA_STRLIBNAME, luaopen_string, 1);
    lua_pop(L, 1);

    if (luaL_dofile(L, path) != LUA_OK) {
        printf("init: rc.lua error: %s\n", lua_tostring(L, -1));
        lua_close(L);
        return -1;
    }

    lua_getglobal(L, "services");
    if (!lua_istable(L, -1)) {
        printf("init: rc.lua has no 'services' table\n");
        lua_close(L);
        return -1;
    }

    int n = (int)lua_rawlen(L, -1);
    service_count = 0;
    for (int i = 1; i <= n && service_count < MAX_SERVICES; i++) {
        lua_rawgeti(L, -1, i);
        if (!lua_istable(L, -1)) {
            lua_pop(L, 1);
            continue;
        }

        service_t* svc = &services[service_count];
        svc->name[0] = '\0';
        svc->cmd[0] = '\0';
        svc->required = 0;

        lua_getfield(L, -1, "name");
        if (lua_isstring(L, -1)) {
            strncpy(svc->name, lua_tostring(L, -1), NAME_MAX_LEN - 1);
        }
        lua_pop(L, 1);

        lua_getfield(L, -1, "cmd");
        if (lua_isstring(L, -1)) {
            strncpy(svc->cmd, lua_tostring(L, -1), CMD_MAX_LEN - 1);
        }
        lua_pop(L, 1);

        lua_getfield(L, -1, "required");
        svc->required = lua_toboolean(L, -1);
        lua_pop(L, 1);

        lua_pop(L, 1);  // services[i]

        if (svc->cmd[0] != '\0') {
            service_count++;
        }
    }

    lua_pop(L, 1);  // services
    lua_close(L);

    return service_count > 0 ? 0 : -1;
}

int main(int argc, char* argv[]) {
    if (load_services_from_lua(RC_LUA_PATH) != 0) {
        printf("init: falling back to default service list\n");
        load_default_services();
    }

    for (int i = 0; i < service_count; i++) {
        printf("init: starting %s\n", services[i].name);
        int exit_code = system(services[i].cmd);
        if (exit_code == -1) {
            printf("init: failed to start %s\n", services[i].name);
            if (services[i].required) {
                break;
            }
            continue;
        }
        printf("init: %s exited with %d\n", services[i].name, exit_code);

        if (services[i].required && exit_code != 0) {
            printf("init: required service '%s' failed, halting\n", services[i].name);
            break;
        }
    }

    while (1) {
    }

    return 0;
}
