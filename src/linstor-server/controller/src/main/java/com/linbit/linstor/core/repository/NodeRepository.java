package com.linbit.linstor.core.repository;

import com.linbit.linstor.core.CoreModule;
import com.linbit.linstor.core.identifier.NodeName;
import com.linbit.linstor.core.objects.Node;
import com.linbit.linstor.security.AccessContext;
import com.linbit.linstor.security.AccessDeniedException;
import com.linbit.linstor.security.AccessType;
import com.linbit.linstor.security.ProtectedObject;

/**
 * Provides access to nodes with automatic security checks.
 */
public interface NodeRepository extends ProtectedObject
{
    void requireAccess(AccessContext accCtx, AccessType requested)
        throws AccessDeniedException;

    Node get(AccessContext accCtx, NodeName nodeName)
        throws AccessDeniedException;

    void put(AccessContext accCtx, NodeName nodeName, Node node)
        throws AccessDeniedException;

    void remove(AccessContext accCtx, NodeName nodeName)
        throws AccessDeniedException;

    CoreModule.NodesMap getMapForView(AccessContext accCtx)
        throws AccessDeniedException;
}
