package com.linbit.linstor.security;

import com.linbit.linstor.LinStorException;

/**
 * Thrown to indicate that an AccessContext does not satisfy the conditions
 * to be allowed access to an object
 *
 * @author Robert Altnoeder &lt;robert.altnoeder@linbit.com&gt;
 */
public class FailedAuthorizationException extends LinStorException
{
    public FailedAuthorizationException(String message)
    {
        super(message);
    }

    public FailedAuthorizationException(String message, Throwable cause)
    {
        super(message, cause);
    }

    public FailedAuthorizationException(
        String message,
        String descriptionText,
        String causeText,
        String correctionText,
        String detailsText
    )
    {
        super(message, descriptionText, causeText, correctionText, detailsText, null);
    }

    public FailedAuthorizationException(
        String message,
        String descriptionText,
        String causeText,
        String correctionText,
        String detailsText,
        Throwable cause
    )
    {
        super(message, descriptionText, causeText, correctionText, detailsText, cause);
    }
}
