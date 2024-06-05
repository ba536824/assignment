package com.linbit.linstor.dbcp.migration;

import com.linbit.linstor.DatabaseInfo.DbProduct;
import com.linbit.linstor.dbdrivers.SQLUtils;

import java.sql.Connection;

@SuppressWarnings("checkstyle:typename")
@Migration(
    version = "2020.08.20.10.00",
    description = "Enable auto-diskful-allow-cleanup"
)
public class Migration_2020_08_20_Enable_AutoDiskfulAllowCleanup extends LinstorMigration
{
    private static final String TBL_PROPS_CONTAINERS = "PROPS_CONTAINERS";
    private static final String PROPS_INSTANCE = "PROPS_INSTANCE";
    private static final String PROP_KEY = "PROP_KEY";
    private static final String PROP_VALUE = "PROP_VALUE";

    @Override
    protected void migrate(Connection connection, DbProduct dbProduct) throws Exception
    {
        SQLUtils.runSql(
            connection,
            " INSERT INTO " + TBL_PROPS_CONTAINERS +
                " (" + PROPS_INSTANCE + ", " + PROP_KEY + ", " + PROP_VALUE + ") " +
                " VALUES ('/CTRLCFG', 'DrbdOptions/auto-diskful-allow-cleanup', 'True')"
        );
    }
}
