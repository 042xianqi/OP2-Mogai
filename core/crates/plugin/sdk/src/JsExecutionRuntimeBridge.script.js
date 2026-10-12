/** Installs the structured tool transport and its owning-call lifecycle. */
(function() {
    var root = globalThis;

    /** Converts tool identity and error fields to text. */
    function asString(value) {
        return value == null ? '' : String(value);
    }

    /** Snapshots complete structured requests without invoking accessors or changing JSON value types. */
    function snapshotJsonObject(value) {
        if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('Tool parameters must be a JSON object');
        var ancestors = [], nodes = 0;
        /** Copies each own JSON data property and rejects unsupported or excessive request data. */
        function copy(entry, depth) {
            if (++nodes > 1000000 || depth > 128) throw new TypeError('Tool request exceeds structured depth/node limit');
            if (entry === null || typeof entry === 'string' || typeof entry === 'boolean') return entry;
            if (typeof entry === 'number' && Number.isFinite(entry)) return entry;
            if (typeof entry !== 'object' || (!Array.isArray(entry) && Object.getPrototypeOf(entry) !== Object.prototype && Object.getPrototypeOf(entry) !== null)) throw new TypeError('Tool request contains a non-JSON value');
            if (ancestors.indexOf(entry) !== -1) throw new TypeError('Tool request contains a cyclic value');
            ancestors.push(entry);
            var array = Array.isArray(entry), result = array ? [] : {};
            var keys = Reflect.ownKeys(entry);
            if (array) {
                if (keys.length !== entry.length + 1) throw new TypeError('Tool request contains non-JSON array properties');
                for (var offset = 0; offset < entry.length; offset++) {
                    if (!Object.prototype.hasOwnProperty.call(entry, offset)) throw new TypeError('Tool request contains a non-JSON array hole');
                }
            }
            keys.forEach(function(key) {
                if (array && key === 'length') return;
                if (typeof key !== 'string') throw new TypeError('Tool request contains a non-JSON key');
                var descriptor = Object.getOwnPropertyDescriptor(entry, key);
                if (!descriptor.enumerable || !Object.prototype.hasOwnProperty.call(descriptor, 'value')) throw new TypeError('Tool request contains a non-JSON property');
                Object.defineProperty(result, key, { value: copy(descriptor.value, depth + 1), enumerable: true, writable: true, configurable: true });
            });
            ancestors.pop();
            return result;
        }
        return copy(value, 0);
    }

    /** Copies optional public parameters through the sole structured request validator. */
    function clonePlainObject(value) {
        if (value === undefined) return {};
        return snapshotJsonObject(value);
    }

    /** Rejects callback streaming that the tool execution host does not implement. */
    function validateOptions(options) {
        if (options === undefined) return;
        if (!options || typeof options !== 'object' || Array.isArray(options)) {
            throw new TypeError('Tool options must be an object');
        }
        if (typeof options.onIntermediateResult === 'function') {
            throw new Error('Tool host does not support intermediate-result callbacks');
        }
    }

    /** Resolves the public tool-call overloads into one structured request. */
    function parseToolCallArguments(rawArgs) {
        if (rawArgs.length === 1 && typeof rawArgs[0] === 'object' && rawArgs[0] !== null) {
            validateOptions(rawArgs[0]);
            return {
                type: rawArgs[0].type === undefined ? 'default' : asString(rawArgs[0].type),
                name: asString(rawArgs[0].name),
                params: clonePlainObject(rawArgs[0].params)
            };
        }
        if (rawArgs.length === 1 && typeof rawArgs[0] === 'string') {
            return { type: 'default', name: rawArgs[0], params: {} };
        }
        if ((rawArgs.length === 2 || rawArgs.length === 3) && typeof rawArgs[0] === 'string' &&
            (typeof rawArgs[1] === 'object' || rawArgs[1] === undefined)) {
            validateOptions(rawArgs[2]);
            return { type: 'default', name: rawArgs[0], params: clonePlainObject(rawArgs[1]) };
        }
        if ((rawArgs.length === 3 || rawArgs.length === 4) &&
            typeof rawArgs[0] === 'string' && typeof rawArgs[1] === 'string') {
            validateOptions(rawArgs[3]);
            return { type: rawArgs[0], name: rawArgs[1], params: clonePlainObject(rawArgs[2]) };
        }
        throw new TypeError('Invalid toolCall arguments');
    }

    /** Reads a structured result envelope without parsing application strings. */
    function parseToolResult(result) {
        if (!result || typeof result !== 'object' || typeof result.success !== 'boolean') {
            throw new TypeError('Tool host returned an invalid result envelope');
        }
        if (!result.success) {
            throw new Error(asString(result.message));
        }
        return result.data;
    }

    /** Executes a structured host Promise and retains its owner through public continuations. */
    function invokeHostAsync(binding, args, convert) {
        return new Promise(function(resolve, reject) {
            var ownerCallId;
            var retained = false;
            /** Releases the exact reference acquired for this invocation. */
            function releaseReference() {
                if (retained) {
                    retained = false;
                    Promise.resolve().then(function() {
                        root.__operitReleaseCallReference(ownerCallId);
                    });
                }
            }
            try {
                ownerCallId = String(root.__operitCurrentCallId);
                root.__operitRetainCallReference(ownerCallId);
                retained = true;
                binding.apply(undefined, [ownerCallId].concat(args)).then(function(result) {
                    try {
                        root.__operitActivateCall(ownerCallId);
                        resolve(convert ? convert(result) : result);
                    } catch (error) {
                        reject(error);
                    } finally {
                        releaseReference();
                    }
                }, function(error) {
                    try {
                        root.__operitActivateCall(ownerCallId);
                        reject(error instanceof Error ? error : new Error(asString(error)));
                    } catch (activationError) {
                        reject(activationError);
                    } finally {
                        releaseReference();
                    }
                });
            } catch (error) {
                reject(error);
                releaseReference();
            }
        });
    }

    /** Resolves the public tool-call overloads into the required structured host binding. */
    function toolCall() {
        try {
            var parsed = parseToolCallArguments(arguments);
            if (typeof root.__operitNativeCallToolStructured !== 'function') {
                throw new Error('Required host binding __operitNativeCallToolStructured is unavailable');
            }
            return invokeHostAsync(root.__operitNativeCallToolStructured,
                [parsed.type, parsed.name, parsed.params, String(root.__operitCurrentCallId)], parseToolResult);
        } catch (error) {
            return Promise.reject(error);
        }
    }

    root.__operitExpose('__operitSnapshotJsonObject', snapshotJsonObject);
    root.__operitExpose('__operitInvokeHostAsync', invokeHostAsync);
    root.__operitExpose('toolCall', toolCall);
})();
